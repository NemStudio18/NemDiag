import requests
import config
import logging

logger = logging.getLogger("NemDiag.Hub.Drivers")

class HubDrivers:
    """Handles hardware driver repository lookups and library updates."""
    
    def __init__(self):
        self.base_url = config.PRO_API_URL
        self.api_key = config.get_api_key()
        self.enabled = config.is_pro()

    def _get_headers(self):
        return {
            "X-API-KEY": self.api_key,
            "User-Agent": f"NemDiag/{config.VERSION}",
            "Content-Type": "application/json"
        }

    def check_drivers(self, components):
        """Check for optimized drivers or library updates from the remote repository."""
        if not self.enabled: return []
        try:
            resp = requests.post(f"{self.base_url}/drivers/check", 
                                json={"components": components}, 
                                headers=self._get_headers())
            return resp.json() if resp.status_code == 200 else []
        except Exception as e:
            logger.error(f"Driver check failed: {e}")
            return []
