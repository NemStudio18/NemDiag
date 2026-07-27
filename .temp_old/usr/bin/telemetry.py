import os
import time
import requests
import config
import logging
import asyncio
import platform
import re

logger = logging.getLogger("NemDiag.Telemetry")

class TelemetryManager:
    """Monitors local logs and reports backend errors to the debug endpoint."""
    
    def __init__(self, log_path=None):
        self.log_path = log_path or config.get_log_path()
        self._last_pos = 0
        self._backoff = 0
        self._last_failure = 0
        self.endpoint = "https://flexcb.fr/api/nemdiag/collect"

    async def start(self):
        """Initializes the log cursor and starts the monitoring loop."""
        # Seek to the end of the log on start to only report NEW errors
        if os.path.exists(self.log_path):
            self._last_pos = os.path.getsize(self.log_path)
            
        logger.info("TelemetryManager started (Log monitoring active).")
        
        while True:
            try:
                # Only check if user has consented in config.json
                if config.get_telemetry_consent():
                    await self._check_logs()
            except Exception as e:
                # Fail silently to avoid being an error source itself
                pass
            
            # Check every 2 minutes (low overhead)
            await asyncio.sleep(120)

    async def _check_logs(self):
        if not os.path.exists(self.log_path): return
        
        current_size = os.path.getsize(self.log_path)
        if current_size < self._last_pos: 
            self._last_pos = 0 # Log was rotated or cleared
            
        if current_size == self._last_pos: return
        
        # Extract new lines
        new_lines = []
        with open(self.log_path, "r") as f:
            f.seek(self._last_pos)
            new_lines = f.readlines()
            self._last_pos = f.tell()
            
        # Filter for ERROR or CRITICAL events
        backend_errors = [line.strip() for line in new_lines if " - ERROR - " in line or " - CRITICAL - " in line]
        
        if backend_errors:
            # SANITIZATION: Clean logs before they leave the machine (GDPR)
            sanitized_errors = [self._sanitize_log(line) for line in backend_errors]
            logger.info(f"Telemetry: Found {len(sanitized_errors)} new backend errors. Attempting report (Sanitized)...")
            await self._report_errors(sanitized_errors)

    def _sanitize_log(self, line):
        """Removes sensitive local information using regex to comply with GDPR."""
        # 1. Hide local paths (/home/user, /mnt, /media)
        line = re.sub(r'/(home|mnt|media|root)/[a-zA-Z0-9._-]+', r'/\1/[USER_HIDDEN]', line)
        # 2. Hide low-level device paths (/dev/sda, /dev/nvme0n1...)
        line = re.sub(r'/dev/[a-zA-Z0-9._-]+', '[DEV_HIDDEN]', line)
        # 3. Hide PIDs (Process IDs) often found in debug logs
        line = re.sub(r'PID:?\s*\d+', 'PID: [HIDDEN]', line, flags=re.IGNORECASE)
        # 4. Hide potentially sensitive hostnames or machine names
        # (Already handled by hash_id in config, but extra safety in logs)
        return line

    async def _report_errors(self, errors):
        """Sends collected errors to the debug endpoint with backoff management."""
        if self._backoff > 0 and (time.time() - self._last_failure) < self._backoff:
            return

        payload = {
            "machine_id": config.get_machine_id(),
            "os": f"{platform.system()} {platform.release()} (Backend)",
            "version": config.VERSION,
            "type": "backend_automatic_report",
            "errors": "\n".join(errors[-20:]) # Send last 20 errors to avoid huge payloads
        }
        
        try:
            # We use to_thread to keep the async loop responsive during HTTP call
            resp = await asyncio.to_thread(
                requests.post, 
                self.endpoint, 
                json=payload, 
                headers={"User-Agent": f"NemDiag-Telemetry/{config.VERSION}"},
                timeout=10
            )
            
            if resp.status_code == 200:
                self._backoff = 0 # Reset on success
                logger.info("Telemetry: Backend error report sent successfully.")
            else:
                self._apply_backoff()
        except:
            self._apply_backoff()

    def _apply_backoff(self):
        """Increase wait time after failure to prevent network spamming."""
        self._last_failure = time.time()
        if self._backoff == 0: 
            self._backoff = 60 # 1 minute
        else: 
            self._backoff = min(self._backoff * 2, 3600) # Max 1 hour
        logger.warning(f"Telemetry: Sync failed. Entering backoff for {self._backoff}s")
