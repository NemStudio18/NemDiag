document.addEventListener('DOMContentLoaded', () => {
    // --- App State ---
    let isReviewMode = false;
    let ws = null;
    let historyCache = [];
    let scoreHistory = [];
    let compareTargets = []; // [id1, id2]

    // --- Telemetry & Error Logging ---
    let errorBuffer = [];
    const originalError = console.error;
    console.error = function(...args) {
        errorBuffer.push(`[ERROR] ${new Date().toISOString()}: ${args.join(' ')}`);
        if (errorBuffer.length > 50) errorBuffer.shift();
        originalError.apply(console, args);
    };

    const consentCheck = document.getElementById('telemetry-consent');
    
    // Initial Load from Server Config (with version busting)
    fetch('/api/config?v=0.2.0')
        .then(r => r.json())
        .then(conf => {
            if (consentCheck) consentCheck.checked = conf.telemetry_consent;
        });

    if (consentCheck) {
        consentCheck.onchange = async () => {
            await fetch('/api/config/telemetry', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ consent: consentCheck.checked })
            });
        };
    }

    async function sendTelemetry(diagData) {
        if (!consentCheck || !consentCheck.checked) return;
        try {
            const payload = {
                os: `${diagData.info.os.distro} ${diagData.info.os.release}`,
                health_score: diagData.health_score,
                intensity: diagData.intensity,
                cpu_model: diagData.info.cpu.model,
                ram_total: diagData.info.ram.total,
                errors: errorBuffer.join('\n')
            };
            await fetch('https://flexcb.fr/api/nemdiag/collect', {
                method: 'POST',
                mode: 'cors',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(payload)
            });
        } catch (e) { /* ignore */ }
    }

    // --- Chart Classes ---
    class MultiChart {
        constructor(canvasId) {
            this.canvas = document.getElementById(canvasId);
            this.ctx = this.canvas ? this.canvas.getContext('2d') : null;
            this.series = {
                cpu: { color: '#818cf8', data: [], visible: true },
                tcpu: { color: '#f87171', data: [], visible: false },
                ram: { color: '#22d3ee', data: [], visible: true },
                gpu: { color: '#a78bfa', data: [], visible: true },
                tgpu: { color: '#f43f5e', data: [], visible: false }
            };
            this.maxPoints = 60;
        }

        addData(cpu, tcpu, ram, gpu, tgpu) {
            if (!this.ctx) return;
            this.series.cpu.data.push(cpu);
            this.series.tcpu.data.push(tcpu);
            this.series.ram.data.push(ram);
            this.series.gpu.data.push(gpu);
            this.series.tgpu.data.push(tgpu);

            Object.values(this.series).forEach(s => {
                if (s.data.length > this.maxPoints) s.data.shift();
            });
            this.draw();
        }
        
        toggle(key, visible) {
            if (this.series[key]) {
                this.series[key].visible = visible;
                this.draw();
            }
        }

        draw() {
            if (!this.canvas || this.canvas.clientWidth === 0) return;
            const w = this.canvas.width = this.canvas.clientWidth * window.devicePixelRatio;
            const h = this.canvas.height = this.canvas.clientHeight * window.devicePixelRatio;
            const ctx = this.ctx;
            ctx.scale(window.devicePixelRatio, window.devicePixelRatio);
            ctx.clearRect(0, 0, this.canvas.clientWidth, this.canvas.clientHeight);
            
            ctx.strokeStyle = '#ffffff0a';
            ctx.lineWidth = 1;
            ctx.beginPath();
            for(let i=1; i<4; i++) {
                const y = this.canvas.clientHeight * (i/4);
                ctx.moveTo(0, y);
                ctx.lineTo(this.canvas.clientWidth, y);
            }
            ctx.stroke();

            const step = this.canvas.clientWidth / (this.maxPoints - 1);
            Object.keys(this.series).forEach(key => {
                const s = this.series[key];
                if (!s.visible || s.data.length < 2) return;
                
                ctx.strokeStyle = s.color;
                ctx.lineWidth = 2.5;
                ctx.lineJoin = 'round';
                ctx.beginPath();
                
                s.data.forEach((d, i) => {
                    const x = i * step;
                    const y = this.canvas.clientHeight - (d / 100 * (this.canvas.clientHeight - 10)) - 5;
                    if (i === 0) ctx.moveTo(x, y);
                    else {
                        const prevX = (i - 1) * step;
                        const prevY = this.canvas.clientHeight - (s.data[i-1] / 100 * (this.canvas.clientHeight - 10)) - 5;
                        ctx.bezierCurveTo(prevX + step/2, prevY, x - step/2, y, x, y);
                    }
                });
                ctx.stroke();
                
                ctx.lineTo((s.data.length - 1) * step, this.canvas.clientHeight);
                ctx.lineTo(0, this.canvas.clientHeight);
                const grad = ctx.createLinearGradient(0, 0, 0, this.canvas.clientHeight);
                grad.addColorStop(0, s.color + '44');
                grad.addColorStop(1, s.color + '00');
                ctx.fillStyle = grad;
                ctx.fill();
            });
        }
    }

    class TrendChart {
        constructor(canvasId, color) {
            this.canvas = document.getElementById(canvasId);
            this.ctx = this.canvas ? this.canvas.getContext('2d') : null;
            this.color = color;
        }

        draw(data) {
            if (!this.ctx || !data || data.length === 0) return;
            const w = this.canvas.width = this.canvas.clientWidth * window.devicePixelRatio;
            const h = this.canvas.height = this.canvas.clientHeight * window.devicePixelRatio;
            const ctx = this.ctx;
            ctx.scale(window.devicePixelRatio, window.devicePixelRatio);
            ctx.clearRect(0, 0, this.canvas.clientWidth, this.canvas.clientHeight);

            ctx.strokeStyle = this.color;
            ctx.lineWidth = 3;
            ctx.lineJoin = 'round';
            ctx.beginPath();
            const step = this.canvas.clientWidth / (Math.max(1, data.length - 1));
            
            data.forEach((d, i) => {
                const x = i * step;
                const y = this.canvas.clientHeight - (d / 100 * (this.canvas.clientHeight - 40)) - 20;
                if (i === 0) ctx.moveTo(x, y);
                else {
                    const prevX = (i - 1) * step;
                    const prevY = this.canvas.clientHeight - (data[i-1] / 100 * (this.canvas.clientHeight - 40)) - 20;
                    ctx.bezierCurveTo(prevX + step/2, prevY, x - step/2, y, x, y);
                }
            });
            ctx.stroke();

            data.forEach((d, i) => {
                const x = i * step;
                const y = this.canvas.clientHeight - (d / 100 * (this.canvas.clientHeight - 40)) - 20;
                ctx.fillStyle = "#fff";
                ctx.beginPath();
                ctx.arc(x, y, 4, 0, Math.PI * 2);
                ctx.fill();
                ctx.strokeStyle = this.color;
                ctx.stroke();
            });
        }
    }

    const multiChart = new MultiChart('multiChart');
    const historyChart = new TrendChart('scoreHistoryChart', '#10b981');
    
    // Toggles for multi chart
    ['cpu', 'tcpu', 'ram', 'gpu', 'tgpu'].forEach(k => {
        const el = document.getElementById(`toggle-${k}`);
        if(el) el.addEventListener('change', (e) => multiChart.toggle(k, e.target.checked));
    });

    // --- Tab Management ---
    const tabBtns = document.querySelectorAll('.tab-btn');
    tabBtns.forEach(btn => {
        btn.addEventListener('click', () => {
            tabBtns.forEach(b => b.classList.remove('active'));
            btn.classList.add('active');
            const target = btn.dataset.tab;
            document.querySelectorAll('.tab-content').forEach(c => c.classList.add('hidden'));
            const targetEl = document.getElementById(target);
            if (targetEl) {
                targetEl.classList.remove('hidden');
                if (target === 'dashboard-tab') {
                    requestAnimationFrame(() => {
                        multiChart.draw();
                        historyChart.draw(scoreHistory);
                    });
                } else if (target === 'history-tab') {
                    updateScoreHistory();
                } else if (target === 'status-tab') {
                    checkSystemStatus();
                }
            }
        });
    });

    // --- WebSocket ---
    let cachedState = {};
    let staticDataReceived = false;
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    function connectWS() {
        ws = new WebSocket(`${protocol}//${window.location.host}/ws`);
        ws.onmessage = (event) => {
            const data = JSON.parse(event.data);
            
            // individual value updates for better reactivity
            if (data.cpu) {
                if (document.getElementById('val-cpu')) document.getElementById('val-cpu').textContent = (data.cpu.usage || 0).toFixed(1) + '%';
                if (document.getElementById('temp-cpu')) {
                    const t = data.cpu.temp || "N/A";
                    document.getElementById('temp-cpu').textContent = (t !== "N/A") ? `(${t})` : '';
                    if (!staticDataReceived && t !== "N/A" && document.getElementById('toggle-tcpu')) {
                        document.getElementById('toggle-tcpu').checked = true;
                        multiChart.toggle('tcpu', true);
                    }
                }
            }
            if (data.ram) {
                if (document.getElementById('val-ram')) document.getElementById('val-ram').textContent = (data.ram.percent || 0).toFixed(1) + '%';
            }
            if (data.gpu && data.gpu.length > 0) {
                const g = data.gpu[0];
                if (document.getElementById('val-gpu')) document.getElementById('val-gpu').textContent = g.usage || 'N/A';
                if (document.getElementById('temp-gpu')) {
                    const gt = g.temp || "N/A";
                    document.getElementById('temp-gpu').textContent = (gt !== "N/A") ? `(${gt})` : '';
                    if (!staticDataReceived && gt !== "N/A" && document.getElementById('toggle-tgpu')) {
                        document.getElementById('toggle-tgpu').checked = true;
                        multiChart.toggle('tgpu', true);
                    }
                }
            }

            // Nouveaux compteurs : Disque et Réseau (reactivity boost)
            if (data.disks && data.disks.length > 0) {
                 const d = data.disks[0];
                 if (document.getElementById('val-disk')) document.getElementById('val-disk').textContent = d.percent + '% (' + (d.used || d.total || '') + ')';
            }
            if (data.network && data.network.length > 0) {
                const n = data.network[0];
                if (document.getElementById('val-net')) document.getElementById('val-net').textContent = n.status === 'Actif' ? (n.ip || 'OK') : 'Déconnecté';
            }
            
            
            // Merge into cached state for full visibility
            cachedState = { ...cachedState, ...data };
            if (data.cpu && cachedState.cpu) cachedState.cpu = { ...cachedState.cpu, ...data.cpu };
            if (data.ram && cachedState.ram) cachedState.ram = { ...cachedState.ram, ...data.ram };

            if (data.os || data.bios) {
                if (document.getElementById('os-summary')) {
                    document.getElementById('os-summary').textContent = `${data.os.distro} | Kernel ${data.os.release}`;
                }
                if (!isReviewMode) displayDetailedConfig(cachedState);
                staticDataReceived = true;
            }

            // Chart update logic
            let gpuVal = 0, tGpuVal = 0;
            if (data.gpu && data.gpu.length > 0) {
                if (data.gpu[0].usage && data.gpu[0].usage !== "N/A") gpuVal = parseFloat(data.gpu[0].usage.replace('%', '')) || 0;
                if (data.gpu[0].temp && data.gpu[0].temp !== "N/A") tGpuVal = parseFloat(data.gpu[0].temp.replace('°C', '')) || 0;
            }
            let tCpuVal = 0;
            if (data.cpu && data.cpu.temp && data.cpu.temp !== "N/A") tCpuVal = parseFloat(data.cpu.temp.replace('°C', '')) || 0;
            
            if (data.cpu && data.ram) {
                multiChart.addData(data.cpu.usage || 0, tCpuVal, data.ram.percent || 0, gpuVal, tGpuVal);
            }
            
            
            if (data.top_cpu && !isReviewMode) {
                updateTopProcs(data);
            }
        };
        ws.onclose = () => { 
            staticDataReceived = false; 
            console.warn("WebSocket closed. Retrying in 3s...");
            setTimeout(connectWS, 3000); 
        };
        ws.onerror = (err) => {
            console.error("WebSocket Error:", err);
        };
    }
    connectWS();

    function updateTopProcs(data) {
        if (document.getElementById('top-cpu-list')) {
            document.getElementById('top-cpu-list').innerHTML = data.top_cpu.map(p => `
                <li class="proc-item"><span>${p.name}</span> <span class="proc-val">${p.usage}%</span></li>
            `).join('');
            document.getElementById('top-ram-list').innerHTML = data.top_ram.map(p => `
                <li class="proc-item"><span>${p.name}</span> <span class="proc-val">${p.mem}</span></li>
            `).join('');
        }
    }

    function displayDetailedConfig(data) {
        const grid = document.getElementById('config-grid');
        grid.innerHTML = `
            <div class="config-card">
                <h4><i class="fa-solid fa-server"></i> Système & Carte Mère</h4>
                <div class="config-item"><span>Hôte</span><span>${data.os.hostname}</span></div>
                <div class="config-item"><span>Distrib</span><span>${data.os.distro}</span></div>
                <div class="config-item"><span>Kernel</span><span>${data.os.release}</span></div>
                <div class="config-item" style="border-top:1px solid rgba(255,255,255,0.1); padding-top:0.5rem; margin-top:0.5rem">
                    <span>Modèle CM</span><span>${data.bios?.vendor || ''} ${data.bios?.product || ''}</span>
                </div>
                <div class="config-item"><span>BIOS</span><span>${data.bios?.bios_version || ''} (${data.bios?.bios_date || ''})</span></div>
            </div>
            <div class="config-card">
                <h4><i class="fa-solid fa-microchip"></i> Processeur</h4>
                <div class="config-item"><span>Modèle</span><span title="${data.cpu?.model || ''}">${(data.cpu?.model || 'Inconnu').substring(0, 30)}...</span></div>
                <div class="config-item"><span>Cœurs</span><span>${data.cpu?.cores_physical || 0}P / ${data.cpu?.cores_logical || 0}L</span></div>
                <div class="config-item"><span>Max Freq</span><span>${data.cpu?.freq_max || 'N/A'}</span></div>
                <div class="config-item" style="flex-direction:column; align-items:flex-start">
                    <span style="opacity:0.6; font-size:0.85em; margin-bottom:0.2rem">Cache & V-Cache</span>
                    <span style="font-size:0.9em; line-height:1.4">${data.cpu?.cache?.replace(/ \| /g, '<br>') || 'N/A'}</span>
                </div>
            </div>
            <div class="config-card">
                <h4><i class="fa-solid fa-memory"></i> RAM & Swap</h4>
                <div class="config-item"><span>RAM Totale</span><span>${data.ram?.total || 'N/A'}</span></div>
                <div class="config-item"><span>Swap</span><span>${data.swap?.total || 'N/A'}</span></div>
                <div class="config-item"><span>Type</span><span>${data.ram?.type || 'DDR'} @ ${data.ram?.speed || 'N/A'}</span></div>
            </div>
            <div class="config-card">
                <h4><i class="fa-solid fa-gamepad"></i> Cartes Graphiques</h4>
                <div style="display:flex; flex-direction:column; gap:0.5rem; margin-top:0.5rem">
                ${(data.gpu || []).map(g => `
                    <div style="background:rgba(255,255,255,0.05); padding:0.8rem; border-radius:8px;">
                        <div style="font-weight:600; margin-bottom:0.2rem">${g?.name || 'GPU Inconnu'}</div>
                        <div style="font-size:0.85em; opacity:0.8; display:flex; justify-content:space-between">
                            <span>💻 ${g?.driver || 'N/A'}</span>
                            <span>📦 ${g?.vram_total || 'Partagée'}</span>
                        </div>
                    </div>
                `).join('') || '<div class="config-item"><span>Aucune détectée</span></div>'}
                </div>
            </div>
            <div class="config-card">
                <h4><i class="fa-solid fa-network-wired"></i> Cartes Réseau & Pilotes</h4>
                <div style="display:flex; flex-direction:column; gap:0.5rem; margin-top:0.5rem">
                ${(data.network || []).map(n => `
                    <div style="background:rgba(255,255,255,0.05); padding:0.8rem; border-radius:8px;">
                        <div style="font-weight:600; margin-bottom:0.2rem">${n.interface || 'Inconnu'}</div>
                        <div style="font-size:0.85em; opacity:0.8; display:flex; justify-content:space-between">
                            <span style="color:#818cf8">⚙️ ${n.driver || 'Générique'}</span>
                            <span>📡 ${n.status || 'N/A'}</span>
                        </div>
                    </div>
                `).join('') || '<div class="config-item"><span>Aucune interface trouvée.</span></div>'}
                </div>
            </div>
            <div class="config-card">
                <h4><i class="fa-solid fa-battery-half"></i> Batterie</h4>
                ${data.battery && data.battery.percent !== undefined ? `
                <div class="config-item">
                    <span>Charge</span>
                    <span style="color:${data.battery.percent > 20 ? '#22c55e' : '#f43f5e'}; font-weight:700">
                        ${data.battery.percent.toFixed(0)}% ${data.battery.plugged ? '<i class="fa-solid fa-plug"></i>' : ''}
                    </span>
                </div>
                ` : '<div class="config-item"><span>Non détectée (Desktop)</span></div>'}
                <div class="config-item" style="border-top:1px solid rgba(255,255,255,0.1); padding-top:0.5rem; margin-top:0.5rem; display:flex; flex-direction:column; align-items:flex-start">
                    <span style="opacity:0.6; font-size:0.85em; margin-bottom:0.2rem"><i class="fa-solid fa-wifi"></i> Connexion Primaire</span>
                    <span style="font-size:0.9em; font-weight:bold; color:var(--primary)">${data.wifi?.ssid && data.wifi.ssid !== 'N/A' ? `WIFI: ${data.wifi.ssid} (${data.wifi.signal || ''} | ${data.wifi.quality || ''})` : 'LAN (Filaire)'}</span>
                </div>
            </div>
            <div class="config-card" style="grid-column: 1 / -1;">
                <h4><i class="fa-solid fa-hard-drive"></i> Stockage Principal</h4>
                <div style="display:grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap:1rem; margin-top:1rem">
                ${(data.disks || []).map(d => `
                    <div style="background:rgba(255,255,255,0.05); padding:1rem; border-radius:12px; border:1px solid rgba(255,255,255,0.1)">
                        <div style="display:flex; justify-content:space-between; margin-bottom:0.8rem">
                            <h5 style="margin:0; font-size:1.05em">${d.name}</h5>
                            <span style="font-size:0.8em; padding:0.2rem 0.6rem; background:rgba(255,255,255,0.15); border-radius:12px; font-weight:600">${d.type}</span>
                        </div>
                        <div class="config-item" style="border-bottom:none; padding:0.2rem 0"><span>Modèle</span><span title="${d.model}">${d.model.substring(0,20)}</span></div>
                        <div class="config-item" style="border-bottom:none; padding:0.2rem 0"><span>Taille</span><span>${d.total}</span></div>
                        <div class="config-item" style="border-bottom:none; padding:0.2rem 0; margin-top:0.5rem; border-top:1px solid rgba(255,255,255,0.1); padding-top:0.5rem">
                            <span>État SMART</span><span>${d.smart_health} ${d.smart_temp !== 'N/A' ? '('+d.smart_temp+')' : ''}</span>
                        </div>
                    </div>
                `).join('')}
                </div>
            </div>
        `;
        
        const usbList = document.getElementById('usb-list');
        if (usbList) {
            usbList.innerHTML = (data.usb || []).map(u => `
                <div class="usb-card" style="display:flex; flex-direction:column; justify-content:space-between">
                    <h4 style="margin:0 0 0.5rem 0; font-size:0.95em" title="${u.name}"><i class="fa-brands fa-usb"></i> ${u.name.substring(0, 35)}</h4>
                    <div>
                        <div style="font-weight:700; color:#818cf8; margin-bottom:0.2rem">${u.speed}</div>
                        <div style="font-size:0.75em; opacity:0.6; display:flex; justify-content:space-between">
                            <span>ID: ${u.vendor_id}:${u.product_id}</span>
                            <span>Bus ${u.bus}</span>
                        </div>
                    </div>
                </div>
            `).join('') || '<p style="opacity:0.7">Aucun périphérique USB connecté.</p>';
        }

        if (document.getElementById('top-cpu-list')) {
            document.getElementById('top-cpu-list').innerHTML = (data.top_cpu || []).map(p => `
                <li class="proc-item"><span>${p.name}</span> <span class="proc-val">${p.usage}%</span></li>
            `).join('');
            document.getElementById('top-ram-list').innerHTML = (data.top_ram || []).map(p => `
                <li class="proc-item"><span>${p.name}</span> <span class="proc-val">${p.mem}</span></li>
            `).join('');
        }
    }

    function showModal(title, content) {
        return new Promise((resolve) => {
            const overlay = document.getElementById('status-modal-overlay');
            if(!overlay) return resolve(true);
            
            document.getElementById('modal-title').innerText = title;
            document.getElementById('modal-content').innerHTML = content;
            overlay.classList.remove('hidden');
            
            const confirmBtn = document.getElementById('modal-confirm');
            const cancelBtn = document.getElementById('modal-cancel');
            
            const onConfirm = () => {
                overlay.classList.add('hidden');
                confirmBtn.removeEventListener('click', onConfirm);
                cancelBtn.removeEventListener('click', onCancel);
                resolve(true);
            };
            const onCancel = () => {
                overlay.classList.add('hidden');
                confirmBtn.removeEventListener('click', onConfirm);
                cancelBtn.removeEventListener('click', onCancel);
                resolve(false);
            };
            confirmBtn.addEventListener('click', onConfirm);
            cancelBtn.addEventListener('click', onCancel);
        });
    }

    async function runDiag(intensity) {
        if (intensity === 'Standard' || intensity === 'Ultra') {
            const conf = await showModal(`Scan ${intensity}`, `<p>Ce test va solliciter vos composants.</p>`);
            if (!conf) return;
        }

        const overlay = document.getElementById('overlay');
        const overlayMsg = document.getElementById('overlay-msg');
        overlay.style.display = 'flex';
        document.querySelectorAll('.step').forEach(s => s.className = 'step');

        const results = {};
        const steps = ['network', 'disk', 'ram', 'cpu', 'gpu'];
        for (const s of steps) {
            const el = document.getElementById(`step-${s}`);
            if (el) el.classList.add('active');
            overlayMsg.textContent = `Vérification ${s}...`;
            try {
                let resData;
                if (s === 'gpu') {
                    resData = await runGPUStressWebGL(intensity, cachedState.gpu || []);
                    results['gpu_stress'] = resData;
                    if (el) el.innerHTML += ` <span style="font-size:0.7em; opacity:0.8">(${resData.estimated_fps} FPS)</span>`;
                } else {
                    const res = await fetch(`/api/run-step/${s}?intensity=${intensity}`);
                    resData = await res.json();
                    results[s === 'cpu' ? 'cpu_stress' : (s === 'ram' ? 'ram_stress' : s)] = resData;
                }
                if (el) el.classList.replace('active', 'done');
            } catch (e) { 
                if(el) {
                    el.classList.remove('active');
                    el.innerHTML += ` <span style="color:#ef4444; font-size:0.7em">(Échec)</span>`;
                }
            }
        }

        overlayMsg.textContent = "Génération du rapport...";
        const info = await (await fetch('/api/info')).json();
        const saveRes = await fetch('/api/save-diagnostic', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({ info, diagnostics: results, intensity })
        });
        const final = await saveRes.json();
        
        overlay.style.display = 'none';
        showDiagnostic(final);
        const banner = document.getElementById('limited-mode-banner');
        if (banner) banner.classList.add('hidden'); // Clear previous banners
        
        updateScoreHistory();
        sendTelemetry(final);
        document.getElementById('score-circle').scrollIntoView({ behavior: 'smooth' });
    }

    function showDiagnostic(final) {
        const breakdown = final.score_breakdown || {};
        const score = breakdown.global ?? final.health_score ?? 0;
        const components = breakdown.components || {};
        const warnings = final.warnings || breakdown.warnings || [];

        document.getElementById('health-score').textContent = score;
        updateScoreColor(score);

        const recList = document.getElementById('rec-list');
        const recBox = document.getElementById('recommendations');
        recList.innerHTML = '';
        
        const insights = breakdown.insights || [];
        const hw_events = breakdown.hw_events || [];

        if (Object.keys(components).length) {
            const labels = { cpu: 'CPU', ram: 'RAM', disk: 'Disque', network: 'Réseau' };
            Object.entries(components).forEach(([key, val]) => {
                const color = val >= 80 ? '#22c55e' : val >= 60 ? '#f59e0b' : '#ef4444';
                recList.innerHTML += `<li><strong>${labels[key] || key}</strong>: <span style="color:${color}">${val}/100</span></li>`;
            });
        }
        warnings.forEach(w => { recList.innerHTML += `<li style="margin-top:.3rem">${w}</li>`; });
        
        if (insights.length || hw_events.length) {
            recList.innerHTML += `<li style="border-top:1px solid rgba(255,255,255,0.1); margin-top:0.8rem; padding-top:0.8rem; list-style:none; font-weight:700; color:var(--primary); letter-spacing:1px">🧠 ANALYSE EXPERTE</li>`;
            hw_events.forEach(e => { 
                recList.innerHTML += `<li style="color:var(--warning); margin-bottom:0.4rem">${e}</li>`; 
                // Show floating alert on dashboard if HW change detected
                const dashTitle = document.querySelector('.config-section h3');
                if (dashTitle && !document.getElementById('hw-alert-icon')) {
                    dashTitle.innerHTML += ` <span id="hw-alert-icon" title="Changement matériel détecté !" style="color:var(--warning); cursor:help; font-size:1.2rem">⚠️</span>`;
                }
            });
            insights.forEach(i => { recList.innerHTML += `<li style="margin-bottom:0.4rem">${i}</li>`; });
        }
        
        if (recList.innerHTML) recBox.classList.remove('hidden');
    }

    function updateScoreColor(score) {
        const circle = document.getElementById('score-circle');
        if (!circle) return;
        circle.style.borderColor = score > 80 ? 'var(--success)' : (score > 60 ? 'var(--warning)' : 'var(--danger)');
    }

    async function updateScoreHistory() {
        const historyList = document.getElementById('history-list');
        if (!historyList) return;
        
        try {
            const res = await fetch('/api/diagnostics');
            const data = await res.json();
            historyCache = data;
            
            const scores = data.map(d => d.health_score || 0).reverse();
            if (document.querySelector('.tab-btn.active')?.dataset.tab === 'dashboard-tab' && historyChart) {
                historyChart.draw(scores.slice(-20));
            }
            
            if (!Array.isArray(data) || data.length === 0) {
                historyList.innerHTML = '<p class="text-muted" style="text-align:center; padding:2rem">Aucun historique disponible.</p>';
                return;
            }
            
            historyList.innerHTML = data.map(d => {
                const diagData = typeof d.data === 'string' ? JSON.parse(d.data) : (d.data || {});
                // Extraction robuste des scores
                const globalScore = d.health_score || (diagData.score_breakdown ? diagData.score_breakdown.global : 100);
                const comps = diagData.score_breakdown ? diagData.score_breakdown.components : (diagData.diagnostics || {});
                
                const components = {
                    cpu: comps.cpu ?? comps.cpu_stress?.score ?? d.cpu_score ?? '--',
                    ram: comps.ram ?? comps.ram_stress?.score ?? d.ram_score ?? '--',
                    gpu: comps.gpu ?? comps.gpu_stress?.score ?? d.gpu_score ?? '--',
                    disk: comps.disk ?? d.disk_score ?? '--'
                };
                
                const isSelected = compareTargets.includes(d.id);
                return `
                <div class="history-item card glass-inner" style="display:flex; justify-content:space-between; align-items:center; transition: 0.3s; margin-bottom: 1rem; padding: 1.2rem; border: ${isSelected ? '1px solid var(--primary)' : '1px solid transparent'}">
                    <div>
                        <div style="font-weight:600; color:var(--primary)">${d.intensity === 'Ultra' ? '🚀 Scan Ultra' : (d.intensity === 'Standard' ? '⚡ Scan Standard' : '🔎 Scan Quick')}</div>
                        <div class="small text-muted">${d.timestamp}</div>
                        <div style="font-size:0.85em; opacity:0.9; line-height:1.4; margin-top:0.4rem; display:flex; gap:0.5rem; flex-wrap:wrap">
                            <span class="badge" style="background:rgba(255,255,255,0.1)">🧠 CPU: ${components.cpu}</span>
                            <span class="badge" style="background:rgba(255,255,255,0.1)">💾 RAM: ${components.ram}</span>
                            <span class="badge" style="background:rgba(255,255,255,0.1)">🎮 GPU: ${components.gpu}</span>
                            <span class="badge" style="background:rgba(255,255,255,0.1)">💿 Disk: ${components.disk}</span>
                        </div>
                    </div>
                    <div style="text-align:right">
                        <div style="font-size:1.4em; font-weight:800">${globalScore} <span style="font-size:0.5em; opacity:0.6">pts</span></div>
                        <div style="display:flex; gap:0.5rem; margin-top:0.4rem">
                            <button class="banner-btn" style="padding:0.3rem 0.6rem; font-size:0.75rem; background:${isSelected ? 'var(--primary)' : ''}" onclick="toggleCompare(${d.id})">${isSelected ? 'DÉSÉLECTIONNER' : 'COMPARER'}</button>
                            <button class="banner-btn secondary" style="padding:0.3rem 0.6rem; font-size:0.75rem" onclick="viewDiagDetail(${d.id})">CONSULTER</button>
                        </div>
                    </div>
                </div>`;
            }).join('');

            const existing = document.getElementById('floating-compare-btn');
            if (existing) existing.remove();

            if (compareTargets.length === 2) {
                const btn = document.createElement('button');
                btn.id = 'floating-compare-btn';
                btn.className = 'main-scan-btn';
                btn.style = 'position:fixed; bottom:2rem; left:50%; transform:translateX(-50%); width:auto; padding:1rem 2rem; z-index:100; box-shadow:0 10px 30px rgba(0,0,0,0.5); border: 2px solid var(--primary)';
                btn.innerHTML = `<i class="fa-solid fa-right-left"></i> Comparer les 2 Scans Sélectionnés`;
                btn.onclick = () => runComparison();
                document.body.appendChild(btn);
            }
        } catch (e) {
            console.error("History fail:", e);
        }
    }

    window.toggleCompare = (id) => {
        if (compareTargets.includes(id)) {
            compareTargets = compareTargets.filter(t => t !== id);
        } else {
            if (compareTargets.length >= 2) compareTargets.shift();
            compareTargets.push(id);
        }
        updateScoreHistory();
    };

    function runComparison() {
        const d1 = historyCache.find(d => d.id === compareTargets[0]);
        const d2 = historyCache.find(d => d.id === compareTargets[1]);
        if (!d1 || !d2) return;
        
        // Sorting by date (oldest first for "Before/After")
        const scans = [d1, d2].sort((a,b) => new Date(a.timestamp) - new Date(b.timestamp));
        const [before, after] = scans;
        
        let html = `<div class="comparison-grid" style="display:grid; grid-template-columns: 1fr 1fr; gap:1.5rem; margin-top:1rem">
            <div style="flex:1">
                <center><div class="small text-muted" style="margin-bottom:0.5rem">📉 RÉFÉRENCE (AVANT)<br><small>${before.timestamp}</small></div><div style="font-size:2.5em; font-weight:800; color:#94a3b8">${before.health_score}</div></center>
            </div>
            <div style="flex:1">
                <center><div class="small text-muted" style="margin-bottom:0.5rem">📈 RELEVÉ (APRÈS)<br><small>${after.timestamp}</small></div><div style="font-size:2.5em; font-weight:800; color:var(--primary)">${after.health_score}</div></center>
            </div>
        </div>
        <div style="margin-top:2rem">`;
        
        const metrics = [
            { label: 'Score Global de Santé', k1: before.health_score, k2: after.health_score },
            { label: 'Performance Processeur (CPU)', k1: before.cpu_score || 0, k2: after.cpu_score || 0 },
            { label: 'Bande Passante Mémoire (RAM)', k1: before.ram_score || 0, k2: after.ram_score || 0 },
            { label: 'Capacités Graphiques (GPU WebGL)', k1: before.gpu_score || 0, k2: after.gpu_score || 0 },
            { label: 'Performance Disque (E/S)', k1: before.disk_score || 0, k2: after.disk_score || 0 }
        ];
        
        metrics.forEach(m => {
            const diff = m.k2 - m.k1;
            const pct = m.k1 > 0 ? ((diff / m.k1) * 100).toFixed(1) : (diff > 0 ? 100 : 0);
            const color = diff > 0 ? '#10b981' : (diff < 0 ? '#f43f5e' : '#94a3b8');
            const sign = diff > 0 ? '+' : '';
            const icon = diff > 0 ? '↗️' : (diff < 0 ? '↘️' : '➡️');
            html += `<div style="display:flex; justify-content:space-between; padding:1.2rem 0; border-bottom:1px solid rgba(255,255,255,0.05)">
                <span style="font-weight:500">${m.label}</span>
                <span style="font-weight:700; color:${color}">${icon} ${sign}${diff} pts (${sign}${pct}%)</span>
            </div>`;
        });
        
        html += `</div>`;
        
        showModal("Rapport de Comparaison Évolutive", html);
    }

    window.reviewHistory = window.viewDiagDetail = (id) => {
        const item = historyCache.find(h => h.id === id);
        if (!item) return;
        isReviewMode = true;
        const data = typeof item.data === 'string' ? JSON.parse(item.data) : (item.data || {});
        
        // Switch to dashboard tab to show the details
        const dashTab = document.querySelector('[data-tab="dashboard-tab"]');
        if (dashTab) {
            dashTab.click();
            setTimeout(() => {
                displayDetailedConfig(data.info || {});
                showDiagnostic(data);
                
                const banner = document.getElementById('limited-mode-banner');
                if (banner) {
                    banner.className = 'banner glass-blue'; 
                    const safeId = parseInt(item.id) || 0;
                    banner.innerHTML = `
                        <div class="banner-content" style="flex:1">
                            <span><i class="fa-solid fa-certificate"></i> <b>Archive Pro</b> : Diagnostic du ${item.timestamp}</span>
                        </div>
                        <div style="display:flex; gap:0.5rem">
                            <button class="banner-btn" style="background:#8b5cf6" onclick="window.open('/api/report/presale/${safeId}', '_blank')"><i class="fa-solid fa-file-contract"></i> CERTIFICAT VENTE</button>
                            <button class="banner-btn" style="background:var(--success)" onclick="setAsBaseline(${safeId})"><i class="fa-solid fa-star"></i> RÉFÉRENCE</button>
                            <button class="banner-btn" style="background:var(--primary)" onclick="printReport(${safeId})"><i class="fa-solid fa-print"></i> PDF</button>
                            <button class="banner-btn secondary" onclick="window.location.reload()">Quitter</button>
                        </div>`;
                    banner.classList.remove('hidden');
                }
            }, 100);
        }
    };

    
    async function securePost(url, data = {}) {
        const resp = await fetch(url, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(data)
        });
        return resp.json();
    }

    async function handleElevation() {
        const conf = await showModal("Expert Mode", "<p>Rédémarrage avec privilèges (pkexec)...</p>");
        if (!conf) return;
        const res = await securePost('/api/elevate');
        if (res.status === 'ok') {
            document.getElementById('overlay').style.display = 'flex';
            document.getElementById('overlay-msg').textContent = "Attente de l'autorisation système...";
            
            async function pollServer() {
                try {
                    const check = await fetch('/api/system-status');
                    if (check.ok) {
                        window.location.reload();
                        return;
                    }
                } catch (e) {
                    // Serveur en cours de redémarrage ou pkexec en attente
                }
                setTimeout(pollServer, 1000);
            }
            pollServer();
        } else {
            alert("Erreur: " + res.message);
        }
    }

    const eb = document.getElementById('btn-elevate');
    const seb = document.getElementById('status-btn-elevate');
    if(eb) eb.onclick = handleElevation;
    if(seb) seb.onclick = handleElevation;

    async function checkSystemStatus() {
        try {
            const res = await fetch('/api/system-status');
            const s = await res.json();
            const banner = document.getElementById('limited-mode-banner');
            if (s.is_root) {
                banner.classList.add('hidden');
                document.getElementById('root-desc').innerText = "Expert Mode Actif.";
                if(seb) seb.classList.add('hidden');
            } else {
                banner.classList.remove('hidden');
                document.getElementById('root-desc').innerText = "Mode Standard.";
                if(seb) seb.classList.remove('hidden');
            }

            const depsUI = document.getElementById('deps-list-ui');
            if (depsUI && s.dependencies) {
                let html = `<h4>Outils Système</h4>`;
                html += Object.entries(s.dependencies).map(([name, active]) => `
                    <div class="status-item card glass-inner" style="margin-top:0.5rem; display:flex; justify-content:space-between; align-items:center">
                        <div class="status-info">
                            <span class="status-dot ${active ? 'green' : 'red'}"></span>
                            <div>
                                <strong>Outil : ${name}</strong>
                                <p class="small text-muted">${active ? 'Installé et détecté.' : 'Manquant.'}</p>
                            </div>
                        </div>
                        ${(!active && s.is_root) ? `<button class="banner-btn" style="background:var(--success); font-size:0.75rem" onclick="window.installTool('${name}')">Récupérer</button>` : ''}
                    </div>
                `).join('');
                
                // Add Drivers section if we have them
                const info = await (await fetch('/api/info')).json();
                if (info.network && info.network.length > 0) {
                    html += `<h4 class="mt-4">Pilotes Matériels</h4>`;
                    html += info.network.map(n => `
                        <div class="status-item card glass-inner" style="margin-top:0.5rem">
                            <div class="status-info">
                                <span class="status-dot ${n.driver !== 'N/A' ? 'green' : 'red'}"></span>
                                <div>
                                    <strong>${n.interface}</strong>
                                    <p class="small text-muted">Pilote : ${n.driver}</p>
                                </div>
                            </div>
                        </div>
                    `).join('');
                }
                depsUI.innerHTML = html;
            }
        } catch (e) {}
    }
    checkSystemStatus();
    setInterval(checkSystemStatus, 60000);
    
    document.getElementById('scan-quick').onclick = () => runDiag('Quick');
    document.getElementById('scan-standard').onclick = () => runDiag('Standard');
    document.getElementById('scan-ultra').onclick = () => runDiag('Ultra');
    
    window.installTool = async (name) => {
        if (!confirm(`Installer le paquet associé à ${name} ?`)) return;
        document.body.style.cursor = 'wait';
        try {
            const res = await securePost(`/api/install-tool/${name}`);
            alert(res.message);
            checkSystemStatus();
        } catch (e) {
            alert("Erreur lors de l'installation.");
        }
        document.body.style.cursor = 'default';
    };

    /**
     * GPU Stress Test via WebGL
     * @param {string} intensity - Quick, Standard, Ultra
     */
    async function runGPUStressWebGL(intensity, gpuInfo = []) {
        const canvas = document.getElementById('gpu-stress-canvas');
        if (!canvas) return { status: 'error', message: 'Canvas non trouvé' };
        
        const gl = canvas.getContext('webgl2') || canvas.getContext('webgl');
        if (!gl) return { status: 'error', message: 'WebGL non supporté' };

        let duration = 5000;
        let iter = 32;
        
        // Detect Integrated GPU (iGPU) to reduce shader iterations and prevent browser lag
        const isIGPU = gpuInfo.some(g => {
            const name = (g.name || "").toLowerCase();
            return name.includes("intel") || name.includes("uhd") || name.includes("iris") || name.includes("integrated") || name.includes("graphics");
        });

        if (intensity === 'Standard') { 
            duration = 12000; 
            iter = isIGPU ? 48 : 64; 
        } else if (intensity === 'Ultra') { 
            duration = 25000; 
            iter = isIGPU ? 64 : 128; 
        }

        const vsSource = `attribute vec4 a_position; void main() { gl_Position = a_position; }`;
        const fsSource = `
            precision highp float;
            uniform float u_time;
            uniform vec2 u_res;
            uniform int u_iter;

            float sdSphere(vec3 p, float s) { return length(p) - s; }
            float sdBox(vec3 p, vec3 b) { vec3 d = abs(p) - b; return min(max(d.x,max(d.y,d.z)),0.0) + length(max(d,0.0)); }
            
            float map(vec3 p) {
                float d1 = sdSphere(p - vec3(sin(u_time)*0.5, 0., 0.), 0.6);
                float d2 = sdBox(p + vec3(0.5, sin(u_time*0.5)*0.3, 0.), vec3(0.4));
                return mix(d1, d2, sin(u_time)*0.5+0.5);
            }

            void main() {
                vec2 uv = (gl_FragCoord.xy - 0.5 * u_res) / u_res.y;
                vec3 ro = vec3(0, 0, -2);
                vec3 rd = normalize(vec3(uv, 1));
                float t = 0.;
                for(int i=0; i<256; i++) {
                    if(i >= u_iter) break;
                    vec3 p = ro + rd * t;
                    float d = map(p);
                    if(d < 0.001 || t > 10.) break;
                    t += d;
                }
                vec3 col = vec3(0);
                if(t < 10.) {
                    vec3 p = ro + rd * t;
                    vec2 e = vec2(0.01, 0);
                    vec3 n = normalize(vec3(map(p+e.xyy)-map(p-e.xyy), map(p+e.yxy)-map(p-e.yxy), map(p+e.yyx)-map(p-e.yyx)));
                    float diff = max(0., dot(n, normalize(vec3(1,2,-3))));
                    col = vec3(0.5 + 0.5*n) * diff;
                    col += pow(max(0., dot(n, normalize(vec3(0,0,-1)))), 32.); // Specular
                }
                gl_FragColor = vec4(col, 0.4);
            }
        `;

        const createShader = (gl, type, source) => {
            const s = gl.createShader(type);
            gl.shaderSource(s, source);
            gl.compileShader(s);
            return s;
        };

        const program = gl.createProgram();
        gl.attachShader(program, createShader(gl, gl.VERTEX_SHADER, vsSource));
        gl.attachShader(program, createShader(gl, gl.FRAGMENT_SHADER, fsSource));
        gl.linkProgram(program);
        gl.useProgram(program);

        const posBuffer = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, posBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1, 1,-1, -1,1, -1,1, 1,-1, 1,1]), gl.STATIC_DRAW);
        const posLoc = gl.getAttribLocation(program, "a_position");
        gl.enableVertexAttribArray(posLoc);
        gl.vertexAttribPointer(posLoc, 2, gl.FLOAT, false, 0, 0);

        const timeLoc = gl.getUniformLocation(program, "u_time");
        const resLoc = gl.getUniformLocation(program, "u_res");
        const iterLoc = gl.getUniformLocation(program, "u_iter");

        canvas.style.display = 'block';
        const start = performance.now();
        let frames = 0;

        return new Promise((resolve) => {
            function render() {
                const now = performance.now();
                if (now - start > duration) {
                    canvas.style.display = 'none';
                    resolve({
                        status: 'ok',
                        intensity: intensity,
                        duration: duration / 1000 + 's',
                        estimated_fps: (frames / (duration / 1000)).toFixed(1)
                    });
                    return;
                }

                gl.uniform1f(timeLoc, (now - start) / 1000);
                gl.uniform2f(resLoc, canvas.width, canvas.height);
                gl.uniform1i(iterLoc, iter);
                gl.drawArrays(gl.TRIANGLES, 0, 6);
                
                frames++;
                requestAnimationFrame(render);
            }
            render();
        });
    }

    // --- Cloud Linking & Sync Logic ---
    let cloudPollInterval = null;

    async function checkCloudStatus() {
        try {
            const res = await fetch('/api/cloud-status');
            const data = await res.json();
            
            // 1. Update Config Tab Widget
            const container = document.getElementById('pro-status-container');
            
            // 2. Update History Tab Banner
            const historyList = document.getElementById('history-list');
            const oldBanner = document.getElementById('cloud-history-banner');
            if (oldBanner) oldBanner.remove();

            if (data.status !== 'linked' && historyList) {
                const banner = document.createElement('div');
                banner.id = 'cloud-history-banner';
                banner.className = 'card glass-inner';
                banner.style = 'margin-bottom:1.5rem; border:1px dashed var(--primary); padding:1rem; display:flex; justify-content:space-between; align-items:center';
                banner.innerHTML = `
                    <div style="font-size:0.9rem">
                        <i class="fa-solid fa-cloud-upload" style="color:var(--primary)"></i> 
                        <strong>Mode Local</strong> : Vos diagnostics ne sont pas sauvegardés en ligne.
                    </div>
                    <button class="banner-btn" style="background:var(--primary)" onclick="initiateCloudLink()">LIER MON COMPTE</button>
                `;
                historyList.prepend(banner);
            }

            if (!container) return;

            if (data.status === 'linked') {
                container.innerHTML = `
                    <div style="display:flex; align-items:center; gap:1rem; background:rgba(16,185,129,0.1); padding:0.5rem 1rem; border-radius:8px; border:1px solid rgba(16,185,129,0.2)">
                        <span style="color:#10b981; font-weight:bold"><i class="fa-solid fa-circle-check"></i> Cloud Pro Actif</span>
                        <button id="btn-sync-history" class="banner-btn" style="padding:0.3rem 0.6rem; font-size:0.75rem; background:var(--primary)">
                            <i class="fa-solid fa-cloud-arrow-up"></i> SYNCHRONISER L'HISTORIQUE
                        </button>
                    </div>`;
                const btn = document.getElementById('btn-sync-history');
                if (btn) btn.onclick = syncHistory;
                
                if (cloudPollInterval) {
                    clearInterval(cloudPollInterval);
                    cloudPollInterval = null;
                }
            } else if (data.status === 'pending') {
                container.innerHTML = `<button class="main-scan-btn secondary" style="width:auto; padding:0.6rem 1.2rem; font-size:0.9rem" onclick="initiateCloudLink()"><i class="fa-solid fa-spinner fa-spin"></i> En attente d'approbation...</button>`;
            } else {
                container.innerHTML = `<button id="btn-link-device" class="main-scan-btn primary" style="width:auto; padding:0.6rem 1.2rem; font-size:0.9rem"><i class="fa-solid fa-link"></i> Lier au Cloud Pro</button>`;
                const btn = document.getElementById('btn-link-device');
                if (btn) btn.onclick = initiateCloudLink;
            }
        } catch(e) { console.error("Cloud status error:", e); }
    }

    window.initiateCloudLink = async () => {
        try {
            const res = await fetch('/api/cloud-link');
            const data = await res.json();
            if (data.url) {
                window.open(data.url, '_blank');
                if (!cloudPollInterval) {
                    checkCloudStatus();
                    cloudPollInterval = setInterval(checkCloudStatus, 3000);
                }
            }
        } catch(e) { alert("Erreur de liaison."); }
    };

    async function syncHistory() {
        const btn = document.getElementById('btn-sync-history');
        if (!btn) return;
        btn.disabled = true;
        btn.innerHTML = '<i class="fa-solid fa-spinner fa-spin"></i> Synchronisation...';
        
        try {
            const resp = await fetch('/api/cloud/sync-history', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' }
            });
            const data = await resp.json();
            if (data.status === 'ok') {
                showToast(`Succès: ${data.synced_count} rapports synchronisés.`);
            } else {
                showToast(`Erreur: ${data.message}`, 'error');
            }
        } catch(e) {
            showToast('Erreur de communication', 'error');
        } finally {
            btn.disabled = false;
            btn.innerHTML = '<i class="fa-solid fa-cloud-arrow-up"></i> SYNCHRONISER L\'HISTORIQUE';
        }
    }

    checkCloudStatus();
    updateScoreHistory();
});

    window.setAsBaseline = async (id) => {
        const resp = await securePost(`/api/config/baseline/${id}`);
        if (resp.status === 'ok') {
            showModal("Référence Enregistrée", "<p>Ce scan est maintenant votre référence de performance officielle pour ce PC.</p>");
        }
    };

    window.printReport = (id) => {
        window.open(`/api/report/print/${id}`, '_blank');
    };

    window.downloadSupportBundle = async () => {
        const lightMode = confirm("🛡️ Mode Privacy-First ?\n\nCliquez sur OK pour expurger les données sensibles (processus, détails).\nCliquez sur ANNULER pour un bundle complet.");
        const mode = lightMode ? 'light' : 'full';
        window.location.href = `/api/support-bundle?mode=${mode}`;
    };
