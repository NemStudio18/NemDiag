        const { invoke } = window.__TAURI__.core;

        let memTotalMb = 0;
        let realtimeInterval = null;
        let lastReport = null; // T8: store last test report for percentile

        // T3/T9/T10: Leaderboard state
        let leaderboardData = [];
        let lbSortKey = 'cpu_score';
        let lbSortAsc = false;

        let cpuHistory = Array(30).fill(0);
        let ramHistory = Array(30).fill(0);

        function drawSparkline(id, historyData, maxVal) {
            let path = document.getElementById(id);
            if (!path) return;
            let d = `M0,40 `;
            for(let i = 0; i < historyData.length; i++) {
                let x = (i / (historyData.length - 1)) * 300;
                let y = 40 - (Math.min(historyData[i] / maxVal, 1) * 40);
                d += `L${x},${y} `;
            }
            d += `L300,40 Z`;
            path.setAttribute('d', d);
        }

        function switchDashTab(tabId) {
            document.querySelectorAll('.dash-tab').forEach(el => el.classList.remove('active'));
            document.querySelectorAll('.dash-content').forEach(el => el.classList.remove('active'));
            if (window.event && window.event.target) {
                window.event.target.classList.add('active');
            } else {
                // Fallback: active tab by tabId
                const tabs = document.querySelectorAll('.dash-tab');
                tabs.forEach(t => {
                    if (t.getAttribute('onclick') && t.getAttribute('onclick').includes(tabId)) {
                        t.classList.add('active');
                    }
                });
            }
            document.getElementById('dash-' + tabId).classList.add('active');
        }

        function switchTab(tabId) {
            document.querySelectorAll('.view').forEach(el => el.classList.remove('active'));
            document.querySelectorAll('.nav-item').forEach(el => el.classList.remove('active'));
            document.getElementById('view-' + tabId).classList.add('active');
            document.getElementById('nav-' + tabId).classList.add('active');
            if (tabId === 'dashboard' || tabId === 'diagnostics') startRealtimeUpdates();
            else stopRealtimeUpdates();
            if (tabId === 'leaderboard') loadLeaderboard();
        }

        // T7/T9/T10/T12: Full leaderboard with sort, filter, normalization
        async function loadLeaderboard() {
            const tbody = document.getElementById('leaderboard-body');
            tbody.innerHTML = '<tr><td colspan="9" style="text-align:center;">Chargement...</td></tr>';
            try {
                const response = await fetch("https://diag-nem.flexcb.fr/api/telemetry.php");
                if (!response.ok) throw new Error("Erreur réseau");
                const raw = await response.json();

                // T10: Filter parasitic data (test entries, all-zero scores)
                leaderboardData = raw.filter(e =>
                    e.os_name !== 'test' && e.cpu_name !== 'test' &&
                    (e.cpu_score > 0 || e.gpu_score > 0 || e.ram_score > 0 || e.disk_score > 0)
                ).map(e => ({
                    ...e,
                    // T9: Normalized score per core
                    cpu_per_core: e.core_count > 0 ? Math.round(e.cpu_score / e.core_count) : 0
                }));

                renderLeaderboard();

                // T8: Show percentile banner if we have a last run
                if (lastReport) showPercentile(lastReport.cpu_score, lastReport.core_count);

            } catch (e) {
                console.error(e);
                tbody.innerHTML = '<tr><td colspan="9" style="text-align:center;color:var(--primary);">Erreur de chargement.</td></tr>';
            }
        }

        function sortLeaderboard(key) {
            if (lbSortKey === key) lbSortAsc = !lbSortAsc;
            else { lbSortKey = key; lbSortAsc = false; }
            renderLeaderboard();
        }

        function renderLeaderboard() {
            const sorted = [...leaderboardData].sort((a, b) => {
                const av = a[lbSortKey] ?? 0, bv = b[lbSortKey] ?? 0;
                return lbSortAsc ? (av > bv ? 1 : -1) : (av < bv ? 1 : -1);
            });
            const tbody = document.getElementById('leaderboard-body');
            if (sorted.length === 0) {
                tbody.innerHTML = '<tr><td colspan="9" style="text-align:center;">Aucun score disponible.</td></tr>';
                return;
            }
            tbody.innerHTML = sorted.map((e, i) => `
                <tr>
                    <td>#${i + 1}</td>
                    <td>${e.os_name}</td>
                    <td>${e.cpu_name} (${e.core_count} cœurs)</td>
                    <td>${Math.round(e.memory_total_mb / 1024)}</td>
                    <td><strong style="color:var(--primary)">${e.cpu_score}</strong></td>
                    <td style="color:#8892b0">${e.cpu_per_core}</td>
                    <td>${e.gpu_score}</td>
                    <td>${e.ram_score > 0 ? e.ram_score + ' Mo/s' : '—'}</td>
                    <td>${e.disk_score > 0 ? e.disk_score + ' Mo/s' : '—'}</td>
                </tr>`).join('');
        }

        // T8: Calculate and display percentile
        function showPercentile(cpuScore, coreCount) {
            const scores = leaderboardData.filter(e => e.cpu_score > 0).map(e => e.cpu_score).sort((a, b) => a - b);
            if (scores.length === 0) return;
            const below = scores.filter(s => s <= cpuScore).length;
            const pct = Math.round((below / scores.length) * 100);
            const normalized = coreCount > 0 ? Math.round(cpuScore / coreCount) : cpuScore;
            const banner = document.getElementById('percentile-banner');
            banner.style.display = 'block';
            banner.innerHTML = `🏆 Votre score CPU (<strong>${cpuScore}</strong>) est dans le <strong>top ${100 - pct}%</strong> des ${scores.length} utilisateurs. Score normalisé : <strong>${normalized}/cœur</strong>.`;
        }

        // Format flat terminal outputs into structured Key-Value rows with optional filtering
        function formatKeyValue(text, keepKeys = []) {
            if (!text || typeof text !== 'string') return text;
            const lines = text.split('\n').filter(l => l.includes(':') && l.trim().length > 3);
            if (lines.length === 0) return 'Aucune donnée formatable.';
            
            return lines.filter(line => {
                if (keepKeys.length === 0) return true;
                let key = line.split(':')[0].trim().toLowerCase();
                return keepKeys.some(k => key.includes(k.toLowerCase()));
            }).map(line => {
                let parts = line.split(':');
                let key = parts.shift().trim();
                let val = parts.join(':').trim();
                if (val === 'None' || val === 'Not Specified' || val === 'Unknown' || val === 'No Module Installed') return '';
                return `<div class="kv-row"><div class="kv-key">${key}</div><div class="kv-val">${val}</div></div>`;
            }).join('');
        }

        async function initCompanion() {
            try {
                const companionData = await invoke('get_companion_advice');
                document.getElementById('companion-resources').innerHTML = companionData.resources_advice;
                document.getElementById('companion-thermal').innerHTML = companionData.thermal_advice;
                document.getElementById('companion-drivers').innerHTML = companionData.drivers_advice;
            } catch (e) {
                document.getElementById('companion-resources').innerHTML = "Erreur de chargement du compagnon.";
                document.getElementById('companion-thermal').innerHTML = "Erreur de chargement.";
                document.getElementById('companion-drivers').innerHTML = "Erreur de chargement.";
                console.error(e);
            }
        }

        async function loadInfo() {
            try {
                const info = await invoke('get_sys_info');
                memTotalMb = info.memory_total_mb;
                document.getElementById('system-info').innerText = `${info.os_name} (Kernel ${info.kernel_version}) | ${info.cpu_name} | ${info.memory_total_mb} MB RAM`;
                startRealtimeUpdates();
                initCompanion();
                // Init dashboard as active
                document.getElementById('nav-dashboard').classList.add('active');
                document.getElementById('view-dashboard').classList.add('active');
                let consent = localStorage.getItem('telemetryConsent');
                if (consent === null) document.getElementById('consent-modal').style.display = 'flex';
                else applyConsent(consent === 'true');
                document.getElementById('telemetry-optin').addEventListener('change', e => setConsent(e.target.checked));
                const detailedInfo = await invoke('get_detailed_system_info');
                function formatDetailCards(text) {
                    if (!text || text.trim() === '') return '<div style="color:var(--text-muted);">Non détecté</div>';
                    const lines = text.split('\n').filter(l => l.trim() !== '');
                    let html = '<div style="display:grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap:15px; margin-top:15px;">';
                    lines.forEach(line => {
                        let parts = line.split(':');
                        if(parts.length >= 2) {
                            let key = parts.shift().trim();
                            let val = parts.join(':').trim();
                            html += `<div style="background:var(--card-bg); padding:12px; border-radius:8px; border:1px solid var(--border);">
                                        <div style="font-size:0.75rem; color:var(--text-muted); margin-bottom:5px; text-transform:uppercase; letter-spacing:1px;">${key}</div>
                                        <div style="font-size:1rem; font-weight:600;">${val}</div>
                                     </div>`;
                        } else {
                            html += `<div style="grid-column: 1 / -1; font-weight:600; padding-top:10px; border-top:1px solid var(--border); margin-top:5px; white-space:pre-wrap; font-size:0.9rem;">${line}</div>`;
                        }
                    });
                    html += '</div>';
                    return html;
                }

                try {
                    const disksJson = JSON.parse(detailedInfo.disks_details);
                    let formattedDisks = "";
                    if (disksJson && disksJson.blockdevices) {
                        disksJson.blockdevices.forEach(d => {
                            formattedDisks += `Nom: ${d.name||'N/A'}\nModèle: ${d.model||'N/A'}\nTaille: ${d.size||'N/A'}\nType: ${d.type||'N/A'}\nMontage: ${d.mountpoint||'N/A'}\n\n`;
                        });
                    }
                    document.getElementById('detail-storage-text').innerHTML = formatDetailCards(formattedDisks || detailedInfo.disks_details);
                } catch(e) {
                    document.getElementById('detail-storage-text').innerHTML = formatDetailCards(detailedInfo.disks_details);
                }
                document.getElementById('detail-system-text').innerHTML = formatDetailCards(detailedInfo.system_details);
                document.getElementById('detail-bios-text').innerHTML = formatDetailCards(detailedInfo.bios_details);
                document.getElementById('detail-mb-text').innerHTML = formatDetailCards(detailedInfo.motherboard);
                document.getElementById('detail-cpu-text').innerHTML = formatDetailCards(detailedInfo.cpu_details);
                document.getElementById('detail-network-text').textContent = detailedInfo.network_details;
                document.getElementById('detail-wifi-text').innerHTML = `<i class="fa-solid fa-wifi"></i> ` + detailedInfo.wifi_details;
                document.getElementById('detail-ram-text').innerHTML = formatDetailCards(detailedInfo.ram_details);
                document.getElementById('detail-gpu-text').innerHTML = formatDetailCards(detailedInfo.gpu_details || "Non détecté");
                document.getElementById('detail-battery-text').innerHTML = formatDetailCards(detailedInfo.battery_details || "Pas de batterie détectée");
                document.getElementById('detail-display-text').innerHTML = formatDetailCards(detailedInfo.display_details || "Non détecté");
                document.getElementById('detail-usb-text').innerHTML = formatDetailCards(detailedInfo.usb_details || "Non détecté");
            } catch (e) { console.error(e); }
        }

        function setConsent(isConsenting) {
            localStorage.setItem('telemetryConsent', isConsenting);
            applyConsent(isConsenting);
            document.getElementById('consent-modal').style.display = 'none';
        }
        function applyConsent(isConsenting) {
            document.getElementById('telemetry-optin').checked = isConsenting;
            invoke('set_telemetry_consent', { consent: isConsenting }).catch(console.error);
        }

        async function fetchRealtimeStats() {
            try {
                const stats = await invoke('get_realtime_stats');
                let cpuUsage = stats.cpu_usage.toFixed(1);
                document.getElementById('cpu-gauge').style.width = cpuUsage + '%';
                document.getElementById('cpu-value').innerText = cpuUsage + '%';
                let ramUsedMb = stats.memory_used / 1024 / 1024;
                let ramPct = (ramUsedMb / memTotalMb) * 100;
                document.getElementById('ram-gauge').style.width = ramPct + '%';
                document.getElementById('ram-value').innerText = `${ramUsedMb.toFixed(0)} MB / ${memTotalMb} MB`;
                cpuHistory.shift(); cpuHistory.push(stats.cpu_usage);
                ramHistory.shift(); ramHistory.push(ramPct);
                drawSparkline('cpu-sparkline', cpuHistory, 100);
                drawSparkline('ram-sparkline', ramHistory, 100);
                drawSparkline('test-cpu-sparkline', cpuHistory, 100);
                drawSparkline('test-ram-sparkline', ramHistory, 100);
                
                let activeCpuValue = document.getElementById('test-cpu-value');
                if (activeCpuValue) activeCpuValue.innerText = cpuUsage + '%';
                let activeRamValue = document.getElementById('test-ram-value');
                if (activeRamValue) activeRamValue.innerText = ramPct.toFixed(1) + '%';

                const tempList = document.getElementById('temp-list');
                tempList.innerHTML = '';
                if (stats.temperatures.length === 0) {
                    tempList.innerHTML = '<li style="color:var(--text-muted);">Aucun capteur détecté</li>';
                } else {
                    stats.temperatures.forEach(t => {
                        let li = document.createElement('li');
                        li.innerText = `${t[0]} : ${t[1].toFixed(1)} °C`;
                        if(t[1] > 80) li.style.color = 'var(--primary)';
                        tempList.appendChild(li);
                    });
                }
                
                const fanList = document.getElementById('fan-list');
                if (stats.fan_speeds && stats.fan_speeds.length > 0) {
                    fanList.innerHTML = '';
                    stats.fan_speeds.forEach(f => {
                        let li = document.createElement('li');
                        li.innerHTML = `${f[0]} : <strong>${f[1]} RPM</strong>`;
                        fanList.appendChild(li);
                    });
                } else {
                    fanList.innerHTML = '<li style="color:var(--text-muted);">Aucun ventilateur détecté</li>';
                }
            } catch (e) { console.error(e); }
        }

        function startRealtimeUpdates() {
            if (!realtimeInterval) {
                fetchRealtimeStats();
                realtimeInterval = setInterval(fetchRealtimeStats, 1000);
            }
        }
        function stopRealtimeUpdates() {
            if (realtimeInterval) { clearInterval(realtimeInterval); realtimeInterval = null; }
        }

        function updateStep(stepId, statusText, active) {
            const el = document.getElementById(stepId);
            el.querySelector('.status').innerText = statusText;
            if (active) { el.classList.add('active'); el.classList.remove('done'); }
            else { el.classList.remove('active'); el.classList.add('done'); }
        }

        // T3: Cancel handler
        let cancelled = false;
        function requestCancel() {
            cancelled = true;
            invoke('cancel_test').catch(console.error);
            document.getElementById('btn-cancel').disabled = true;
            document.getElementById('btn-cancel').innerText = 'Annulation...';
            document.getElementById('progress-label').innerText = 'Annulation en cours...';
        }

        // T6: Countdown progress bar
        let progressInterval = null;
        function startProgressBar(label, durationSeconds) {
            clearInterval(progressInterval);
            let elapsed = 0;
            document.getElementById('progress-label').innerHTML = label;
            document.getElementById('time-progress-bar').style.width = '0%';
            progressInterval = setInterval(() => {
                elapsed++;
                const remaining = Math.max(0, durationSeconds - elapsed);
                const pct = Math.min(100, (elapsed / durationSeconds) * 100);
                document.getElementById('time-progress-bar').style.width = pct + '%';
                document.getElementById('progress-time').innerText = remaining > 0 ? `${remaining}s restant` : 'Finalisation...';
                if (elapsed >= durationSeconds) clearInterval(progressInterval);
            }, 1000);
        }
        function stopProgressBar() {
            clearInterval(progressInterval);
            document.getElementById('time-progress-bar').style.width = '100%';
            document.getElementById('progress-time').innerText = 'Terminé';
        }

        async function startDiagnostic(mode, durationSeconds) {
            cancelled = false;
            document.getElementById('test-selection').style.display = 'none';
            document.getElementById('progress-container').style.display = 'block';
            document.getElementById('btn-cancel').disabled = false;
            document.getElementById('btn-cancel').innerHTML = '<i class="fa-solid fa-xmark"></i> Annuler le test';
            // Reset steps
            ['step-cpu','step-gpu','step-ram','step-disk','step-smart'].forEach(id => {
                const el = document.getElementById(id);
                el.classList.remove('active','done');
                el.querySelector('.status').innerText = 'En attente...';
            });

            const overlay = document.getElementById('webgl-overlay');
            const canvas = document.getElementById('webgl-canvas');
            const uiText = document.getElementById('webgl-ui');
            let visualizerInterval = null;
            
            const loaderCPU = ["Comptage de l'infini...", "Chauffe du réacteur nucléaire...", "Recherche de nombres premiers...", "Saturation des cœurs..."];
            const loaderRAM = ["Téléchargement de plus de RAM...", "Remplissage des seaux de bits...", "Écriture massive de zéros...", "Allocation de la matrice..."];
            const loaderDISK = ["Recherche de vos dossiers secrets...", "Gravure au burin des octets...", "Test des limites de la physique...", "Lecture du système de fichiers..."];

            function showVisualizer(type) {
                overlay.style.display = 'block';
                uiText.style.display = 'block';
                let opCount = 0;
                let tickCount = 0;
                if (type === 'GPU') {
                    canvas.style.display = 'block';
                    uiText.innerHTML = `<div style="position: absolute; top: 20px; left: 20px;">[STRESS TEST GPU EN COURS]<br>FPS: <span id="webgl-fps">0</span><br>Calculs fractals : <span id="webgl-ops">0</span> MOp/s</div>`;
                    startWebGLStress();
                    visualizerInterval = setInterval(() => { opCount += Math.random()*500+1000; let el=document.getElementById('webgl-ops'); if(el) el.innerText=Math.floor(opCount); }, 100);
                } else {
                    canvas.style.display = 'none';
                    let title = type==='CPU'?'STRESS TEST CPU':(type==='RAM'?'STRESS TEST RAM':'STRESS TEST DISQUE');
                    let unit = type==='CPU'?'GFlops':(type==='RAM'?'Mo/s':'Mo/s');
                    let msgArray = type==='CPU'?loaderCPU:(type==='RAM'?loaderRAM:loaderDISK);
                    let accentColor = type==='CPU'?'#00bfff':(type==='RAM'?'#7c3aed':'#f59e0b');

                    uiText.innerHTML = `
                        <div class="test-active-ui" style="text-align:center; padding: 1rem;">
                            <div style="font-size:0.85rem; letter-spacing:0.15em; color:#8892b0; margin-bottom:1.5rem;">[${title} EN COURS]</div>
                            
                            <!-- Graphiques CPU/RAM en direct -->
                            <div style="display:flex; justify-content:center; gap:20px; margin-bottom: 20px; width:100%; max-width:400px; margin: 0 auto 1.5rem;">
                                <div style="flex:1;">
                                    <div style="font-size:0.75rem; color:var(--text-muted); margin-bottom:5px; display:flex; justify-content:space-between;">
                                        <span>CPU</span> <span id="test-cpu-value" style="color:var(--text-main); font-weight:bold;">0%</span>
                                    </div>
                                    <svg preserveAspectRatio="none" viewBox="0 0 300 40" style="width:100%; height:30px; background:rgba(0,0,0,0.2); border-radius:4px;">
                                        <path id="test-cpu-sparkline" d="M0,40 L300,40" fill="rgba(255,51,102,0.1)" stroke="var(--primary)" stroke-width="2"/>
                                    </svg>
                                </div>
                                <div style="flex:1;">
                                    <div style="font-size:0.75rem; color:var(--text-muted); margin-bottom:5px; display:flex; justify-content:space-between;">
                                        <span>RAM</span> <span id="test-ram-value" style="color:var(--text-main); font-weight:bold;">0%</span>
                                    </div>
                                    <svg preserveAspectRatio="none" viewBox="0 0 300 40" style="width:100%; height:30px; background:rgba(0,0,0,0.2); border-radius:4px;">
                                        <path id="test-ram-sparkline" d="M0,40 L300,40" fill="rgba(0,230,118,0.1)" stroke="#00e676" stroke-width="2"/>
                                    </svg>
                                </div>
                            </div>

                            <!-- Ring loader animé -->
                            <div style="position:relative; width:120px; height:120px; margin:0 auto 1.5rem;">
                                <svg viewBox="0 0 120 120" style="width:120px;height:120px;transform:rotate(-90deg);">
                                    <circle cx="60" cy="60" r="50" fill="none" stroke="rgba(255,255,255,0.05)" stroke-width="8"/>
                                    <circle id="loader-ring" cx="60" cy="60" r="50" fill="none" stroke="${accentColor}" stroke-width="8"
                                        stroke-dasharray="314" stroke-dashoffset="314"
                                        style="transition: stroke-dashoffset 0.5s ease; filter: drop-shadow(0 0 6px ${accentColor});"/>
                                </svg>
                                <div style="position:absolute;top:50%;left:50%;transform:translate(-50%,-50%);text-align:center;">
                                    <div id="generic-ops" style="font-size:1.5rem;font-weight:800;color:${accentColor};font-family:monospace;">0</div>
                                    <div style="font-size:0.6rem;color:#8892b0;margin-top:2px;">${unit}</div>
                                </div>
                            </div>

                            <!-- Message rotatif -->
                            <div id="loader-msg" style="color:var(--text-muted);font-style:italic;font-size:0.9rem;min-height:1.4em;margin-bottom:1rem;">${msgArray[0]}</div>

                            <!-- Barres de particules animées -->
                            <div style="display:flex;gap:3px;justify-content:center;align-items:flex-end;height:40px;">
                                ${Array.from({length:20},(_,i)=>`<div class="loader-bar-particle" style="width:6px;background:${accentColor};border-radius:3px;opacity:0.7;animation:particleBounce 1.2s ease-in-out ${(i*0.06).toFixed(2)}s infinite alternate;"></div>`).join('')}
                            </div>
                        </div>`;

                    // Injection CSS animation si pas déjà présent
                    if (!document.getElementById('loader-anim-style')) {
                        const s = document.createElement('style');
                        s.id = 'loader-anim-style';
                        s.textContent = `
                            @keyframes particleBounce {
                                from { height: 4px; }
                                to   { height: 36px; }
                            }`;
                        document.head.appendChild(s);
                    }

                    visualizerInterval = setInterval(async () => {
                        tickCount++;
                        if (tickCount % 6 === 0) {
                            let loaderMsg = document.getElementById('loader-msg');
                            if (loaderMsg) loaderMsg.innerText = msgArray[Math.floor(tickCount / 6) % msgArray.length];
                        }

                        let val = 0;
                        if (type === 'RAM') {
                            val = await invoke('get_live_ram_throughput');
                        } else if (type === 'DISK') {
                            val = await invoke('get_live_disk_throughput');
                        } else {
                            opCount += Math.random()*50+10;
                            val = Math.floor(opCount);
                        }

                        let el = document.getElementById('generic-ops');
                        if (el) el.innerText = val;

                        // Rotation ring basée sur la valeur (max arbitraire)
                        let ring = document.getElementById('loader-ring');
                        if (ring && val > 0) {
                            let maxVal = type==='RAM' ? 60000 : type==='DISK' ? 10000 : 1000;
                            let pct = Math.min(1, val / maxVal);
                            ring.setAttribute('stroke-dashoffset', (377 * (1 - pct)).toFixed(1));
                        }
                    }, 500);
                }
            }
            function hideVisualizer() {
                if (visualizerInterval) clearInterval(visualizerInterval);
                stopWebGLStress();
                overlay.style.display = 'none';
            }

            // CPU
            updateStep('step-cpu', `En cours (${durationSeconds}s)...`, true);
            startProgressBar('<i class="fa-solid fa-rotate fa-spin"></i> Stress Test CPU', durationSeconds);
            showVisualizer('CPU');
            await invoke('run_cpu_test', { duration: durationSeconds });
            hideVisualizer(); stopProgressBar();
            updateStep('step-cpu', cancelled ? 'Annulé' : 'Terminé', false);
            if (cancelled) { resetDiagnosticUI(); return; }

            // GPU
            updateStep('step-gpu', `En cours (${durationSeconds}s)...`, true);
            startProgressBar('<i class="fa-solid fa-rotate fa-spin"></i> Stress Test GPU', durationSeconds);
            showVisualizer('GPU');
            await invoke('run_gpu_test', { duration: durationSeconds });
            hideVisualizer(); stopProgressBar();
            updateStep('step-gpu', cancelled ? 'Annulé' : 'Terminé', false);
            if (cancelled) { resetDiagnosticUI(); return; }

            // RAM
            updateStep('step-ram', `En cours (${durationSeconds}s)...`, true);
            startProgressBar('<i class="fa-solid fa-rotate fa-spin"></i> Stress Test RAM', durationSeconds);
            showVisualizer('RAM');
            await invoke('run_ram_test', { duration: durationSeconds });
            hideVisualizer(); stopProgressBar();
            updateStep('step-ram', cancelled ? 'Annulé' : 'Terminé', false);
            if (cancelled) { resetDiagnosticUI(); return; }

            // DISK
            updateStep('step-disk', `En cours (${durationSeconds}s)...`, true);
            startProgressBar('<i class="fa-solid fa-rotate fa-spin"></i> Stress Test Disque', durationSeconds);
            showVisualizer('DISK');
            await invoke('run_disk_test', { duration: durationSeconds });
            hideVisualizer(); stopProgressBar();
            updateStep('step-disk', cancelled ? 'Annulé' : 'Terminé', false);
            if (cancelled) { resetDiagnosticUI(); return; }

            // SMART & REPORT
            updateStep('step-smart', 'Génération du rapport...', true);
            document.getElementById('progress-label').innerHTML = '<i class="fa-solid fa-file-export fa-bounce"></i> Génération du rapport...';
            
            // Get or generate User ID
            let userId = localStorage.getItem('nemdiag_user_id');
            if (!userId) {
                userId = crypto.randomUUID ? crypto.randomUUID() : 'user-' + Date.now() + Math.random().toString(36).substring(2);
                localStorage.setItem('nemdiag_user_id', userId);
            }
            
            let reportJson;
            try {
                const reportRaw = await invoke('run_smart_and_export', { userId: userId });
                reportJson = JSON.parse(reportRaw);
                lastReport = reportJson; // T8: save for percentile
                updateStep('step-smart', 'Terminé', false);
            } catch (e) {
                updateStep('step-smart', 'Erreur : ' + e, false);
                resetDiagnosticUI();
                return;
            }

            // Populate Results
            document.getElementById('score-cpu').innerText = reportJson.cpu_score;
            document.getElementById('score-gpu').innerText = reportJson.gpu_score;
            document.getElementById('score-ram').innerText = reportJson.ram_score + " Mo/s";
            document.getElementById('score-disk').innerText = reportJson.disk_score + " Mo/s";
            document.getElementById('advice-cpu').innerText = reportJson.cpu_advice || "Analyse non disponible.";
            document.getElementById('advice-gpu').innerText = reportJson.gpu_advice || "Analyse non disponible.";
            document.getElementById('advice-ram').innerText = reportJson.ram_advice || "Analyse non disponible.";
            document.getElementById('advice-disk').innerText = reportJson.disk_advice || "Analyse non disponible.";

            function setBadgeColor(id, score, medium, good) {
                let badge = document.getElementById(id);
                if (!badge) return;
                badge.className = 'score-badge';
                if (score < medium) badge.classList.add('status-bad');
                else if (score < good) badge.classList.add('status-warn');
                else badge.classList.add('status-good');
            }
            setBadgeColor('score-cpu', reportJson.cpu_score, 4000, 10000);
            setBadgeColor('score-gpu', reportJson.gpu_score, 300, 1500);
            setBadgeColor('score-ram', reportJson.ram_score, 5000, 12000);
            setBadgeColor('score-disk', reportJson.disk_score, 150, 600);

            let cpu_n = Math.min(reportJson.cpu_score / 120, 100);
            let gpu_n = Math.min(reportJson.gpu_score / 20, 100);
            let ram_n = Math.min(reportJson.ram_score / 120, 100);
            let disk_n = Math.min(reportJson.disk_score / 8, 100);
            let global_score = Math.round(cpu_n * 0.4 + gpu_n * 0.3 + ram_n * 0.15 + disk_n * 0.15);
            let globalScoreEl = document.getElementById('global-score-value');
            globalScoreEl.innerText = global_score;
            if (global_score < 40) globalScoreEl.style.color = '#dc3545';
            else if (global_score < 75) globalScoreEl.style.color = '#ffc107';
            else globalScoreEl.style.color = '#28a745';
            
            if (reportJson.run_id) {
                let webBtn = document.getElementById('web-leaderboard-btn');
                webBtn.href = `https://diag-nem.flexcb.fr/detail.php?id=${reportJson.run_id}`;
                webBtn.style.display = 'inline-block';
            }

            resetDiagnosticUI();
            // T17: Unlock results tab after successful test
            const navResults = document.getElementById('nav-results');
            navResults.style.opacity = '1';
            navResults.style.pointerEvents = 'auto';
            navResults.title = '';
            switchTab('results');
        }

        function resetDiagnosticUI() {
            document.getElementById('test-selection').style.display = 'flex';
            document.getElementById('progress-container').style.display = 'none';
            clearInterval(progressInterval);
        }

        async function exportReport(format) {
            if (!lastReport) {
                alert("Aucun rapport à exporter. Veuillez lancer un test d'abord.");
                return;
            }
            try {
                // Generate a cool export file
                const result = await invoke('export_report', { format: format, reportJson: JSON.stringify(lastReport) });
                alert("Rapport exporté avec succès !\nFichier enregistré sous :\n" + result);
            } catch (e) {
                alert("Erreur lors de l'exportation :\n" + e);
            }
        }

        window.addEventListener('DOMContentLoaded', loadInfo);
    </script>
