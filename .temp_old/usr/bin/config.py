import os
import json
import uuid
import platform
import hashlib

# NemDiag Pro Configuration System
VERSION_FILE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "VERSION")

def get_cpu_model():
    try:
        with open("/proc/cpuinfo", "r") as f:
            for line in f:
                if "model name" in line:
                    return line.split(":")[1].strip()
    except: pass
    return platform.processor()

def generate_secure_machine_id():
    """Generates a stable, anonymized SHA256 hash from system identifiers."""
    sys_id = ""
    # Try multiple sources for a stable system ID (more stable on VMs)
    for path in ["/etc/machine-id", "/var/lib/dbus/machine-id"]:
        if os.path.exists(path):
            with open(path, "r") as f:
                sys_id = f.read().strip()
                break
    
    if not sys_id:
        # Fallback to a stable hardware identifier if machine-id is missing
        sys_id = str(uuid.getnode()) 
        
    cpu = get_cpu_model()
    # SHA256 provides a one-way, stable anonymized ID (GDPR compliant)
    return hashlib.sha256(f"nemdiag-v5-{sys_id}-{cpu}".encode()).hexdigest()

def get_version():
    try:
        with open(VERSION_FILE, "r") as f:
            return f.read().strip()
    except:
        return "v0.2-beta"

VERSION = get_version()
APP_NAME = "NemDiag Pro"
RELEASE_TAG = "Beta Release"
HMAC_SECRET = "nemdiag_pro_v6_enterprise_secret_key" # Will be synced with Hub Pro later

# HUB PRO SETTINGS
PRO_API_URL = "https://nemdiag.test"
REMOTE_VERSION_URL = f"{PRO_API_URL}/latest_version"
# XDG Standard: Config in ~/.config/nemdiag
CONFIG_PATH = os.path.expanduser("~/.config/nemdiag/config.json")
LEGACY_CONFIG_PATH = os.path.expanduser("~/.nemdiag/config.json")

def load_config():
    # MIGRATION: If old path exists and new doesn't, migrate automatically
    if not os.path.exists(CONFIG_PATH) and os.path.exists(LEGACY_CONFIG_PATH):
        os.makedirs(os.path.dirname(CONFIG_PATH), exist_ok=True)
        import shutil
        try:
            shutil.copy2(LEGACY_CONFIG_PATH, CONFIG_PATH)
            # We keep the old one for now but use the new one
        except: pass

    if not os.path.exists(CONFIG_PATH):
        os.makedirs(os.path.dirname(CONFIG_PATH), exist_ok=True)
        
        # New SHA256 stable Machine ID
        machine_id = generate_secure_machine_id()
        initial_config = {"machine_id": machine_id, "linked": False, "api_key": None, "telemetry_consent": False}
        
        fd = os.open(CONFIG_PATH, os.O_WRONLY | os.O_CREAT, 0o600)
        with os.fdopen(fd, 'w') as f:
            json.dump(initial_config, f)
        return initial_config
    
    # Ensure existing file is secured
    try: os.chmod(CONFIG_PATH, 0o600)
    except: pass

    with open(CONFIG_PATH, "r") as f:
        conf = json.load(f)
        
    # Migration: Update to 64-char Hash if using legacy UUID format
    if "machine_id" not in conf or len(conf["machine_id"]) < 64:
        conf["machine_id"] = generate_secure_machine_id()
        with open(CONFIG_PATH, "w") as f:
            json.dump(conf, f)
            
    # Migration: Add telemetry_consent if missing
    if "telemetry_consent" not in conf:
        conf["telemetry_consent"] = False
        with open(CONFIG_PATH, "w") as f:
            json.dump(conf, f)
            
    return conf

def set_linked_status(status: bool):
    conf = load_config()
    conf["linked"] = status
    with open(CONFIG_PATH, "w") as f:
        json.dump(conf, f)

def get_machine_id():
    return load_config().get("machine_id")

def is_pro():
    return load_config().get("linked", False)

def get_api_key():
    return load_config().get("api_key")  # Legacy fallback

def get_telemetry_consent():
    return load_config().get("telemetry_consent", False)

def set_telemetry_consent(status: bool):
    conf = load_config()
    conf["telemetry_consent"] = status
    with open(CONFIG_PATH, "w") as f:
        json.dump(conf, f)

def get_log_path():
    """Returns the XDG-compliant path for nemdiag.log."""
    if platform.system() == "Linux":
        data_dir = os.path.expanduser("~/.local/share/nemdiag")
        os.makedirs(data_dir, exist_ok=True)
        return os.path.join(data_dir, "nemdiag.log")
    return "nemdiag.log"
