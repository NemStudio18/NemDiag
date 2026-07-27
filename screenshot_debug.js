const { chromium } = require('playwright');
const path = require('path');
const fs = require('fs');

(async () => {
    const browser = await chromium.launch({ headless: true });
    const page = await browser.newPage();
    
    const errors = [];
    page.on('console', msg => {
        if (msg.type() === 'error' || msg.type() === 'warning') {
            errors.push(`[${msg.type().toUpperCase()}] ${msg.text()}`);
        }
    });
    
    await page.goto('https://diag-nem.flexcb.fr/', { waitUntil: 'networkidle' });
    await page.screenshot({ path: '/tmp/screen_before_click.png', fullPage: true });
    console.log('Screenshot avant clic sauvegardé');

    // Clic sur la première ligne clickable
    await page.click('tr.clickable-row');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: '/tmp/screen_after_click.png', fullPage: true });
    console.log('Screenshot après clic sauvegardé');
    
    // État du DOM de la modale
    const modalInfo = await page.evaluate(() => {
        const overlay = document.querySelector('[n-id="modal-overlay"]');
        if (!overlay) return 'INTROUVABLE';
        return {
            classes: overlay.className,
            display: overlay.style.display,
            opacity: overlay.style.opacity,
            innerHTML_preview: overlay.innerHTML.substring(0, 200)
        };
    });
    console.log('État modal:', JSON.stringify(modalInfo, null, 2));
    console.log('Erreurs console:', errors.length > 0 ? errors.join('\n') : 'Aucune erreur');
    
    await browser.close();
})();
