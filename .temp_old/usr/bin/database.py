import sqlite3
import json
from datetime import datetime
import os


import platform

def get_db_path():
    """Returns the XDG-compliant path for diagnostics.db with auto-migration."""
    if platform.system() == "Linux":
        data_dir = os.path.expanduser("~/.local/share/nemdiag")
        os.makedirs(data_dir, exist_ok=True)
        new_path = os.path.join(data_dir, "diagnostics.db")
        
        # MIGRATION: If local DB exists but XDG doesn't, migrate it
        # Note: We check if it exists in the current working directory
        old_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "diagnostics.db")
        if os.path.exists(old_path) and not os.path.exists(new_path):
            import shutil
            try:
                shutil.copy2(old_path, new_path)
            except: pass
        return new_path
    return "diagnostics.db"

DB_PATH = get_db_path()
SCORE_ALGO_VERSION = "5.0.0" # Current Enterprise Algorithm

def init_db():
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
            intensity TEXT,
            health_score INTEGER,
            cpu_score INTEGER,
            ram_score INTEGER,
            gpu_score INTEGER,
            disk_score INTEGER,
            data TEXT NOT NULL,
            is_synced INTEGER DEFAULT 0,
            is_baseline INTEGER DEFAULT 0,
            score_algo_version TEXT
        )
    """)
    
    # Table Thermals : Historique des températures pour analyse de dérive (pâte thermique)
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS thermals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
            sensor_name TEXT,
            temp_current REAL,
            temp_max REAL
        )
    """)
    conn.commit()
    conn.close()
    
    # SAFETY: Backup before migration
    if os.path.exists(DB_PATH):
        import shutil
        ts = datetime.now().strftime("%Y%m%d_%H%M%S")
        shutil.copy(DB_PATH, f"{DB_PATH}.bak.{ts}")
        
    migrate_db()

def migrate_db():
    """Adds missing columns and backfills existing JSON data into new SQL columns."""
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    
    # Check current schema
    cursor.execute("PRAGMA table_info(history)")
    existing_cols = [col[1] for col in cursor.fetchall()]
    
    columns = [
        ("intensity", "TEXT"),
        ("health_score", "INTEGER"),
        ("cpu_score", "INTEGER"),
        ("ram_score", "INTEGER"),
        ("gpu_score", "INTEGER"),
        ("disk_score", "INTEGER"),
        ("is_synced", "INTEGER DEFAULT 0"),
        ("is_baseline", "INTEGER DEFAULT 0"),
        ("score_algo_version", "TEXT")
    ]
    
    migrated_any = False
    for col, ctype in columns:
        if col not in existing_cols:
            try:
                cursor.execute(f"ALTER TABLE history ADD COLUMN {col} {ctype}")
                migrated_any = True
            except: pass
            
    # Always try to backfill if we have null scores
    cursor.execute("SELECT id, data FROM history WHERE health_score IS NULL")
    rows = cursor.fetchall()
    
    for row_id, data_str in rows:
        try:
            data = json.loads(data_str)
            intensity = data.get("intensity", "Quick")
            
            # Extract scores safely
            h_score = data.get("health_score", 0)
            if isinstance(h_score, int):
                global_score = h_score
                comps = {}
            else:
                global_score = h_score.get("global", 0)
                comps = h_score.get("components", {})
                
            c_score = comps.get("cpu", 0)
            r_score = comps.get("ram", 0)
            g_score = comps.get("gpu", 0)
            d_score = comps.get("disk", 0)
            
            cursor.execute("""
                UPDATE history 
                SET intensity = ?, health_score = ?, cpu_score = ?, ram_score = ?, gpu_score = ?, disk_score = ?
                WHERE id = ?
            """, (intensity, global_score, c_score, r_score, g_score, d_score, row_id))
        except Exception:
            pass # Skip corrupted JSON blobs naturally
            
    conn.commit()
    conn.close()

def save_diagnostic(data: dict, is_synced=0):
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    
    # Extract scores for dedicated columns
    h_score = data.get("health_score", {})
    if isinstance(h_score, int): global_score = h_score; comps = {}
    else: global_score = h_score.get("global", 0); comps = h_score.get("components", {})
    
    cursor.execute("""
        INSERT INTO history (intensity, health_score, cpu_score, ram_score, gpu_score, disk_score, data, is_synced, score_algo_version) 
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
    """, (
        data.get("intensity", "Quick"),
        global_score,
        comps.get("cpu", 0),
        comps.get("ram", 0),
        comps.get("gpu", 0),
        comps.get("disk", 0),
        json.dumps(data),
        is_synced,
        SCORE_ALGO_VERSION
    ))
    conn.commit()
    conn.close()

def mark_as_synced(row_id):
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute("UPDATE history SET is_synced = 1 WHERE id = ?", (row_id,))
    conn.commit()
    conn.close()

def get_unsynced_history():
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute("SELECT id, timestamp, data FROM history WHERE is_synced = 0 ORDER BY timestamp ASC")
    rows = cursor.fetchall()
    conn.close()
    
    return [{"id": r[0], "timestamp": r[1], "data": json.loads(r[2])} for r in rows]

def set_baseline(diag_id, intensity="Quick"):
    """Marks a specific diagnostic as the baseline (reference) for its intensity category."""
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    # Reset previous baseline for the SAME intensity
    cursor.execute("UPDATE history SET is_baseline = 0 WHERE intensity = ?", (intensity,))
    # Set new baseline
    cursor.execute("UPDATE history SET is_baseline = 1 WHERE id = ?", (diag_id,))
    conn.commit()
    conn.close()

def get_baseline_score(intensity="Quick"):
    """Returns the score of the baseline scan if it exists, otherwise None."""
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute("SELECT health_score FROM history WHERE is_baseline = 1 AND intensity = ?", (intensity,))
    row = cursor.fetchone()
    conn.close()
    return row[0] if row else None

def get_history(limit=50):
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute("SELECT id, timestamp, data, health_score, cpu_score, ram_score, gpu_score, disk_score, intensity, score_algo_version FROM history ORDER BY timestamp DESC LIMIT ?", (limit,))
    rows = cursor.fetchall()
    conn.close()
    
    history = []
    for row in rows:
        try:
            data = json.loads(row[2]) if isinstance(row[2], str) else row[2]
        except: data = {}
        history.append({
            "id": row[0],
            "timestamp": row[1],
            "data": data,
            "health_score": row[3],
            "cpu_score": row[4],
            "ram_score": row[5],
            "gpu_score": row[6],
            "disk_score": row[7],
            "intensity": row[8],
            "score_algo_version": row[9] or "5.0.0-legacy"
        })
    return history

def get_history_export(limit=1000, light=False):
    """Returns history records for export, optionally expunging sensitive blobs."""
    if light:
        conn = sqlite3.connect(DB_PATH)
        cursor = conn.cursor()
        # In light mode, we don't even select the 'data' column (privacy first)
        cursor.execute("""
            SELECT id, timestamp, health_score, cpu_score, ram_score, gpu_score, disk_score, intensity, score_algo_version 
            FROM history ORDER BY timestamp DESC LIMIT ?
        """, (limit,))
        rows = cursor.fetchall()
        conn.close()
        return [{
            "id": r[0], "timestamp": r[1], "health_score": r[2], 
            "cpu_score": r[3], "ram_score": r[4], "gpu_score": r[5], 
            "disk_score": r[6], "intensity": r[7], "score_algo_version": r[8] or "5.0.0",
            "data": "[EXPUNGED FOR PRIVACY]" 
        } for r in rows]
    else:
        # Full mode: uses the standard get_history logic
        return get_history(limit)

def get_latest_scan_score(intensity="Quick"):
    """Returns the global score of the most recent scan of the SAME intensity."""
    if not os.path.exists(DB_PATH): return 100
    try:
        conn = sqlite3.connect(DB_PATH)
        cursor = conn.cursor()
        # Search for intensity in the JSON data field
        cursor.execute("SELECT data FROM history ORDER BY id DESC")
        rows = cursor.fetchall()
        conn.close()
        for row in rows:
            data = json.loads(row[0])
            if data.get("intensity") == intensity:
                return data.get("health_score", {}).get("global", 100)
    except: pass
    return 100

def save_thermal(sensor_name, temp_current, temp_max):
    """Saves a thermal snapshot for long-term trend analysis (paste drying)."""
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute("INSERT INTO thermals (sensor_name, temp_current, temp_max) VALUES (?, ?, ?)", 
                   (sensor_name, temp_current, temp_max))
    conn.commit()
    conn.close()

def get_thermal_history(limit=100):
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute("SELECT timestamp, temp_current, temp_max FROM thermals ORDER BY timestamp DESC LIMIT ?", (limit,))
    rows = cursor.fetchall()
    conn.close()
    return rows

if __name__ == "__main__":
    init_db()
    print("Database initialized.")
