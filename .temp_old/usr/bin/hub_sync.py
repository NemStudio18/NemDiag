import requests
import config
import logging
import time

logger = logging.getLogger("NemDiag.Hub.Sync")

class HubSync:
    """Handles scan synchronization and before/after cloud saving."""
    
    def __init__(self):
        self.base_url = config.PRO_API_URL
        self.machine_id = config.get_machine_id()
        self.enabled = config.is_pro()
        self._backoff = 0
        self._last_failure = 0

    def _get_headers(self):
        return {
            "X-MACHINE-ID": self.machine_id,
            "User-Agent": f"NemDiag/{config.VERSION}",
            "Content-Type": "application/json"
        }

    def sync_scan(self, diagnostic_data):
        if not self.enabled: return False
        
        # Prevent spamming the SaaS if we are in backoff period
        if self._backoff > 0 and (time.time() - self._last_failure) < self._backoff:
            return False

        try:
            # Nouvelle API v5.0
            resp = requests.post(f"{self.base_url}/api/nemdiag/collect", 
                                json=diagnostic_data, 
                                headers=self._get_headers(), 
                                timeout=10)
            
            if resp.status_code == 200:
                self._backoff = 0  # Reset backoff on success
                return True
            else:
                self._apply_backoff()
                return False
        except Exception as e:
            logger.error(f"Sync failed: {e}")
            self._apply_backoff()
            return False

    def _apply_backoff(self):
        """Linearly then exponentially increase wait time (max 1 hour)."""
        self._last_failure = time.time()
        if self._backoff == 0: 
            self._backoff = 30  # Start with 30s
        else:
            self._backoff = min(self._backoff * 2, 3600) 
        logger.warning(f"SaaS Sync: entering backoff for {self._backoff}s")

    def sync_history(self):
        """Send all unsynced local reports to the cloud."""
        if not self.enabled: return 0
        
        import database
        unsynced = database.get_unsynced_history()
        count = 0
        
        for item in unsynced:
            if self.sync_scan(item['data']):
                database.mark_as_synced(item['id'])
                count += 1
            else:
                break # Stop on first failure (likely quota or network)
        
        return count
