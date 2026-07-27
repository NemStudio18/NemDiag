from fastapi import FastAPI, WebSocket, WebSocketDisconnect, Request, Header, HTTPException, Depends, responses
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles
from fastapi.responses import HTMLResponse
from contextlib import asynccontextmanager
import asyncio
import json
import os
import time
import subprocess
import logging
import logging.handlers
import requests
import hashlib
import hmac
from datetime import datetime
import uvicorn

# Project Modules
import config
import hub_sync
import hub_drivers
import telemetry
import database
import processor
import collector
import tester

# Pro Hub Clients
sync_client = hub_sync.HubSync()
drivers_client = hub_drivers.HubDrivers()

# Project Metadata
VERSION = config.VERSION
APP_NAME = config.APP_NAME
RELEASE_TAG = config.RELEASE_TAG

# Base Directory for static assets
BASE_DIR = os.path.dirname(os.path.abspath(__file__))
STATIC_DIR = os.path.join(BASE_DIR, "static")

# Setup Logging
LOG_FILE = config.get_log_path()
handler = logging.handlers.RotatingFileHandler(LOG_FILE, maxBytes=10*1024*1024, backupCount=5)
handler.setFormatter(logging.Formatter('%(asctime)s - %(levelname)s - %(message)s'))
logger = logging.getLogger("NemDiag")
logger.setLevel(logging.INFO)
logger.addHandler(handler)
logger.propagate = False


def send_notification(title, message):
    try:
        icon_path = os.path.join(os.path.dirname(__file__), "static", "icon.png")
        subprocess.run(["notify-send", "-i", icon_path, title, message], check=False)
    except: pass

@asynccontextmanager
async def lifespan(app: FastAPI):
    # Startup: Initialize telemetry and database
    asyncio.create_task(telemetry_mgr.start())
    database.init_db()
    logger.info("NemDiag Backend initialized.")
    yield
    # Shutdown: Add cleanup here if needed in future

app = FastAPI(lifespan=lifespan)
telemetry_mgr = telemetry.TelemetryManager()

# Add CORS Middleware to allow local connections
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

@app.middleware("http")
async def add_cache_control_header(request, call_next):
    response = await call_next(request)
    if request.url.path.endswith("/") or request.url.path.endswith(".html"):
        response.headers["Cache-Control"] = "no-cache, no-store, must-revalidate"
        response.headers["Pragma"] = "no-cache"
        response.headers["Expires"] = "0"
    return response

# Serve static files
if not os.path.exists(STATIC_DIR): os.makedirs(STATIC_DIR)
app.mount("/static", StaticFiles(directory=STATIC_DIR), name="static")

@app.get("/")
async def get():
    index_path = os.path.join(STATIC_DIR, "index.html")
    with open(index_path, "r") as f:
        return HTMLResponse(content=f.read())

@app.get("/api/info")
async def get_system_info():
    return await asyncio.to_thread(collector.get_system_info)

@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    await websocket.accept()
    try:
        # Initial static data (OS, BIOS, etc.)
        static_info = await asyncio.to_thread(collector.get_static_info)
        try:
            await websocket.send_text(json.dumps(static_info))
        except Exception as je:
            logger.error(f"Initial WebSocket serialization error: {je}")
        
        while True:
            # Send only dynamic info (CPU/RAM/GPU usage, temps) every 2 seconds
            dynamic_info = await asyncio.to_thread(collector.get_dynamic_info)
            try:
                await websocket.send_text(json.dumps(dynamic_info))
            except Exception as je:
                logger.error(f"Dynamic WebSocket serialization error: {je}")
            await asyncio.sleep(2)
    except WebSocketDisconnect:
        logger.info("WebSocket disconnected by client.")
    except Exception as e:
        logger.error(f"Websocket loop error: {e}")
    finally:
        # Starlette handles the close message if a disconnect was raised
        pass

@app.get("/api/history")
async def get_diag_history():
    return database.get_history()

@app.get("/api/system-status")
async def get_system_status():
    import shutil
    return {
        "is_root": os.geteuid() == 0,
        "dependencies": {
            "lsusb": shutil.which("lsusb") is not None,
            "lscpu": shutil.which("lscpu") is not None,
            "dmidecode": shutil.which("dmidecode") is not None,
            "smartctl": shutil.which("smartctl") is not None,
            "nvidia-smi": shutil.which("nvidia-smi") is not None
        }
    }

@app.post("/api/elevate")
async def elevate_privileges():
    import sys
    executable = sys.executable
    script_path = os.path.abspath(sys.argv[0])
    cmd = ["pkexec", executable, script_path] + sys.argv[1:]
    try:
        subprocess.Popen(cmd, env=os.environ.copy(), start_new_session=True)
        asyncio.create_task(delayed_exit(6.0)) 
        return {"status": "ok", "message": "Demande d'élévation envoyée (pkexec). (Relancez après élévation)"}
    except Exception as e:
        return {"status": "error", "message": str(e)}

async def delayed_exit(delay=1.5):
    await asyncio.sleep(delay)
    os._exit(0)

@app.get("/api/run-step/{step}")
async def run_single_step(step: str, intensity: str = "Quick"):
    result = await asyncio.to_thread(tester.run_step, step, intensity)
    return {"status": "ok", "result": result}

@app.post("/api/save-diagnostic")
async def save_diagnostic(request: Request):
    data = await request.json()
    # 1. Scoring
    if "health_score" not in data or isinstance(data["health_score"], int):
        score_details = tester.calculate_health_score(data.get("info", {}), data.get("diagnostics", {}))
        data["health_score"] = score_details.get("global", 100)
        data["score_breakdown"] = score_details

    # 2. Regression & Trends
    intensity = data.get("intensity", "Quick")
    baseline_score = database.get_baseline_score(intensity)
    cur_score = data.get("health_score", 100)
    ref_score = baseline_score if baseline_score is not None else database.get_latest_scan_score(intensity)
    
    if ref_score and cur_score < ref_score - 15:
        send_notification("Régression Détectée", f"Baisse de {ref_score - cur_score} pts.")

    # 3. Insights & Analytics
    history = database.get_history(limit=20) # Larger history for trends
    prev_info = history[0].get('data', {}).get('info', {}) if history else {}
    data["score_breakdown"]["insights"] = processor.analyze_upgrade_potential(data.get("diagnostics", {}), data.get("info", {}))
    data["score_breakdown"]["hw_events"] = processor.diff_hardware(data.get('info', {}), prev_info)
    data["score_breakdown"]["warnings"] = processor.watchdog_processes(history)
    
    # New: Add thermal trends to warnings
    thermal_warnings = processor.check_thermal_trends(history)
    data["score_breakdown"]["warnings"].extend(thermal_warnings)

    # 4. Save
    await asyncio.to_thread(database.save_diagnostic, data)
    if config.is_pro(): asyncio.create_task(safe_sync_scan(data))
    return {"status": "saved", "id": data.get("id") or 0, "health_score": data["health_score"]}

@app.get("/api/check-update")
async def check_update():
    """Checks for a new version of NemDiag Pro from the remote repository."""
    try:
        # Avoid blocking the main loop
        resp = await asyncio.to_thread(requests.get, config.REMOTE_VERSION_URL, timeout=5)
        if resp.status_code == 200:
            remote_version = resp.text.strip()
            return {
                "update_available": remote_version != config.VERSION,
                "remote_version": remote_version,
                "local_version": config.VERSION
            }
    except: pass
    return {"update_available": False, "local_version": config.VERSION}

@app.get("/api/scan/auto")
async def auto_scan(request: Request, intensity: str = "Quick"):
    """Local access only check with X-NemDiag-Daemon header."""
    is_daemon = request.headers.get("X-NemDiag-Daemon") == "true"
    is_local = request.client.host == "127.0.0.1"
    if not is_local: raise HTTPException(status_code=403, detail="Local access only")
    asyncio.create_task(run_full_diag(intensity))
    return {"status": "Scan automatique lancé", "daemon_mode": is_daemon}

async def run_full_diag(intensity):
    info = collector.get_system_info()
    results = {}
    for step in ['network', 'disk', 'ram', 'cpu']:
        results[step] = await asyncio.to_thread(tester.run_step, step, intensity)
    
    diag_data = {"info": info, "diagnostics": results, "intensity": intensity}
    
    # 5. Driver Guard Check (SaaS Hub)
    if config.is_pro():
        try:
            # Pass ONLY the GPU list if exists
            gpus = info.get("gpu", [])
            driver_tips = await asyncio.to_thread(drivers_client.check_drivers, gpus)
            if driver_tips:
                if "score_breakdown" not in diag_data: diag_data["score_breakdown"] = {"insights": []}
                if "insights" not in diag_data["score_breakdown"]: diag_data["score_breakdown"]["insights"] = []
                diag_data["score_breakdown"]["insights"].extend(driver_tips)
        except Exception as e:
            logger.error(f"Driver Guard failed: {e}")

    await save_diag(diag_data, None)

async def safe_sync_scan(data):
    try: await asyncio.to_thread(sync_client.sync_scan, data)
    except Exception as e: logger.error(f"Sync fail: {e}")

@app.get("/api/config")
async def get_app_config():
    return {"machine_id": config.get_machine_id(), "linked": config.is_pro(), "telemetry_consent": config.get_telemetry_consent(), "version": config.VERSION}

@app.post("/api/config/telemetry")
async def update_telemetry_consent(request: Request):
    data = await request.json()
    config.set_telemetry_consent(data.get("consent", False))
    return {"status": "ok"}

@app.post("/api/config/baseline/{diag_id}")
async def set_baseline(diag_id: int):
    # Retrieve scan intensity first
    history = database.get_history(limit=500)
    item = next((h for h in history if h["id"] == diag_id), None)
    if not item: return {"status": "error", "message": "Diagnostic non trouvé"}
    
    intensity = item.get("data", {}).get("intensity", "Quick")
    database.set_baseline(diag_id, intensity)
    return {"status": "ok"}

@app.get("/api/report/print/{diag_id}")
async def print_report(diag_id: int):
    # Reuse presale_report logic but with print auto-trigger
    return await presale_report(diag_id)

@app.get("/api/report/presale/{diag_id}")
async def presale_report(diag_id: int):
    history = database.get_history(limit=500)
    item = next((h for h in history if h["id"] == diag_id), None)
    if not item: return HTMLResponse("Diagnostic non trouvé", status_code=404)
    
    # HMAC SIGNATURE
    msg = f"{diag_id}:{item['timestamp']}:{item['health_score']}"
    sig = hmac.new(config.HMAC_SECRET.encode(), msg.encode(), hashlib.sha256).hexdigest()
    
    data = item["data"]
    info = data.get("info", {})
    insights = data.get("score_breakdown", {}).get("insights", [])
    security = collector.get_security_status()
    
    html = f"""
    <!DOCTYPE html>
    <html lang="fr">
    <head>
        <meta charset="UTF-8">
        <title>Certificat NemDiag Pro #{diag_id}</title>
        <style>
            @import url('https://fonts.googleapis.com/css2?family=Outfit:wght@300;400;600;700;800&display=swap');
            body {{ font-family: 'Outfit', sans-serif; background: #f8fafc; padding: 40px; display:flex; justify-content:center; }}
            .cert {{ width: 800px; background: white; padding: 60px; border-top: 10px solid #2563eb; box-shadow: 0 10px 30px rgba(0,0,0,0.05); position:relative; }}
            .score-seal {{ position:absolute; top:40px; right:40px; width:120px; height:120px; border:4px solid #22c55e; border-radius:50%; display:flex; flex-direction:column; align-items:center; justify-content:center; transform:rotate(15deg); color:#22c55e; }}
            .hmac {{ font-family:monospace; font-size:10px; color:#94a3b8; margin-top:40px; border-top:1px dashed #e2e8f0; padding-top:10px; }}
        </style>
    </head>
    <body onload="window.print()">
        <div class="cert">
            <div class="score-seal"><div style="font-size:40px;font-weight:800">{item['health_score']}</div><div>SCORE</div></div>
            <h1>Certificat de Santé NemDiag</h1>
            <p>ID Diagnostic : #{diag_id} | Date : {item['timestamp']}</p>
            <hr>
            <h3>Configuration Audité</h3>
            <p>CPU : {info.get('cpu', {{}}).get('model', 'N/A')}</p>
            <p>RAM : {info.get('ram', {{}}).get('total', 'N/A')}</p>
            <p>OS : {info.get('os', {{}}).get('distro', 'Linux')}</p>
            <hr>
            <h3>Expertise Matérielle</h3>
            <ul>{"".join([f"<li>{i}</li>" for i in insights]) if insights else "<li>Certifié conforme aux spécifications constructeur.</li>"}</ul>
            <h3>Sécurité</h3>
            <p>Pare-feu : {security.get('firewall', 'Non-Configuré')}</p>
            <div class="hmac">SIGNATURE HMAC-SHA256 : {sig}</div>
        </div>
    </body>
    </html>
    """
    return HTMLResponse(html)



@app.get("/api/diagnostics")
async def get_all_diagnostics():
    try:
        history = await asyncio.to_thread(database.get_history, 50)
        return history
    except Exception as e:
        return {"status": "error", "message": str(e)}

@app.post("/api/install-tool/{tool_name}")
async def install_tool(tool_name: str):
    if os.geteuid() != 0:
        return {"status": "error", "message": "Privilèges root requis pour l'installation."}
    pkg_map = {
        "smartctl": "smartmontools",
        "lsusb": "usbutils",
        "lscpu": "util-linux",
        "dmidecode": "dmidecode",
        "nvidia-smi": "nvidia-utils"
    }
    pkg = pkg_map.get(tool_name, tool_name)
    pm = collector.get_package_manager()
    cmd = []
    if pm == "apt": cmd = ["apt-get", "update", "&&", "apt-get", "install", "-y", pkg]
    elif pm == "dnf": cmd = ["dnf", "install", "-y", pkg]
    elif pm == "pacman": cmd = ["pacman", "-S", "--noconfirm", pkg]
    if not cmd:
        return {"status": "error", "message": f"Gestionnaire de paquets '{pm}' non supporté pour l'auto-installation."}
    import subprocess
    try:
        full_cmd = " ".join(cmd) if pm == "apt" else cmd
        subprocess.run(full_cmd, shell=(pm=="apt"), check=True, capture_output=True, timeout=120)
        return {"status": "ok", "message": f"'{tool_name}' (paquet {pkg}) installé avec succès."}
    except Exception as e:
        return {"status": "error", "message": f"Échec de l'installation : {str(e)}"}

@app.get("/api/cloud-link")
async def get_cloud_link():
    machine_id = config.get_machine_id()
    import socket
    hostname = socket.gethostname()
    url = f"{config.PRO_API_URL}/nemdiag/link?machine_id={machine_id}&name={hostname}"
    return {"status": "ok", "machine_id": machine_id, "url": url}

@app.get("/api/cloud-status")
async def check_cloud_status():
    machine_id = config.get_machine_id()
    if config.is_pro(): return {"status": "linked"}
    try:
        import requests
        resp = await asyncio.to_thread(requests.get, f"{config.PRO_API_URL}/api/nemdiag/status?machine_id={machine_id}", timeout=5)
        if resp.status_code == 200:
            data = resp.json()
            if data.get("status") == "linked":
                config.set_linked_status(True)
                return {"status": "linked", "message": "Appareil approuvé par le CMS."}
    except Exception:
        pass
    return {"status": "pending"}

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="NemDiag Enterprise Diagnostic tool")
    parser.add_argument("--scan", action="store_true", help="Executer un scan immédiat en ligne de commande")
    parser.add_argument("--intensity", default="Quick", choices=["Quick", "Standard", "Ultra"], help="Intensité du scan CLI")
    parser.add_argument("--port", type=int, default=8000, help="Port du serveur API")
    
    args = parser.parse_args()
    
    if args.scan:
        print(f"🚀 Lancement du scan CLI (Modèle: {args.intensity})...")
        asyncio.run(run_full_diag(args.intensity))
        print("✅ Scan terminé et enregistré en base locale.")
    else:
        uvicorn.run(app, host="127.0.0.1", port=args.port)
