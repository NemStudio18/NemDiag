<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>NemDiag - Podium</title>
    <link href="https://fonts.googleapis.com/css2?family=Outfit:wght@300;400;600;800&family=JetBrains+Mono:wght@400;700&display=swap" rel="stylesheet">
    <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.4.0/css/all.min.css">
    <style>
        :root {
            --bg-dark: #020617;
            --primary: #00e676;
            --primary-glow: rgba(0, 230, 118, 0.3);
            --accent: #38bdf8;
            --accent-glow: rgba(56, 189, 248, 0.3);
            --card-bg: rgba(15, 23, 42, 0.6);
            --text: #f8fafc;
            --text-dim: #94a3b8;
            --border: rgba(255, 255, 255, 0.06);
        }
        
        * { margin: 0; padding: 0; box-sizing: border-box; }
        
        body {
            font-family: 'Outfit', sans-serif;
            background-color: var(--bg-dark);
            color: var(--text);
            line-height: 1.6;
        }

        .container { max-width: 1200px; margin: 0 auto; padding: 0 1.5rem; }

        nav {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 2rem 0;
            border-bottom: 1px solid var(--border);
            margin-bottom: 3rem;
        }

        .logo {
            font-weight: 900;
            font-size: 1.8rem;
            letter-spacing: -1px;
            display: flex;
            align-items: center;
            gap: 12px;
            background: linear-gradient(to right, var(--primary), var(--accent));
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            text-decoration: none;
        }

        .nav-links { display: flex; gap: 2rem; align-items: center; }
        .nav-links a {
            color: var(--text-dim);
            text-decoration: none;
            font-weight: 500;
            font-size: 0.9rem;
            transition: all 0.3s;
        }
        .nav-links a:hover, .nav-links a.active { color: var(--primary); text-shadow: 0 0 10px var(--primary-glow); }

        .leaderboard-table {
            width: 100%;
            border-collapse: separate;
            border-spacing: 0 8px;
            margin-top: 1rem;
        }

        .leaderboard-table th {
            text-align: left;
            padding: 1rem;
            color: var(--text-muted);
            font-weight: 600;
            text-transform: uppercase;
            font-size: 0.85rem;
            letter-spacing: 1px;
            border-bottom: 1px solid var(--border);
        }

        .leaderboard-table td {
            padding: 1rem;
            background: var(--card-bg);
            border-top: 1px solid var(--border);
            border-bottom: 1px solid var(--border);
        }

        .leaderboard-table tr td:first-child {
            border-left: 1px solid var(--border);
            border-top-left-radius: 8px;
            border-bottom-left-radius: 8px;
        }

        .leaderboard-table tr td:last-child {
            border-right: 1px solid var(--border);
            border-top-right-radius: 8px;
            border-bottom-right-radius: 8px;
        }

        .rank-badge {
            display: inline-flex;
            align-items: center;
            justify-content: center;
            width: 32px;
            height: 32px;
            border-radius: 50%;
            background: rgba(255,255,255,0.1);
            font-weight: 700;
            font-size: 0.9rem;
        }

        .rank-1 .rank-badge { background: #fbbf24; color: #000; box-shadow: 0 0 15px rgba(251, 191, 36, 0.5); }
        .rank-2 .rank-badge { background: #94a3b8; color: #000; }
        .rank-3 .rank-badge { background: #b45309; color: #fff; }

        .score-val {
            font-family: 'JetBrains Mono', monospace;
            font-weight: 700;
            color: var(--primary);
        }

        .spec-item {
            font-size: 0.8rem;
            color: var(--text-muted);
            display: flex;
            align-items: center;
            gap: 5px;
            margin-bottom: 3px;
        }

        .spec-item i { width: 14px; text-align: center; }

        .loading-state {
            text-align: center;
            padding: 4rem;
            color: var(--text-muted);
        }
    </style>
</head>
<body>
    <div class="container">
        <nav>
            <a href="index.php" class="logo"><i class="fa-solid fa-microchip"></i> NemDiag</a>
            <div class="nav-links">
                <a href="index.php">Accueil</a>
                <a href="speedtest.php">Speedtest</a>
                <a href="leaderboard.php" class="active">Podium</a>
            </div>
        </nav>

        <div style="text-align: center; margin-bottom: 2rem;">
            <h2 style="font-size: 2rem; margin-bottom: 0.5rem;"><i class="fa-solid fa-trophy" style="color: #fbbf24;"></i> Leaderboard Global</h2>
            <p style="color: var(--text-dim);">Les meilleures machines testées sur NemDiag.</p>
        </div>

    <div id="leaderboard-container">
        <div class="loading-state">
            <i class="fa-solid fa-circle-notch fa-spin fa-2x"></i>
            <p style="margin-top: 1rem;">Chargement du classement...</p>
        </div>
    </div>

    <script>
        async function fetchLeaderboard() {
            try {
                // Fetching from the same domain API
                const res = await fetch('/api/telemetry.php?top=50');
                if (!res.ok) throw new Error('API Error');
                const data = await res.json();
                
                if (data.length === 0) {
                    document.getElementById('leaderboard-container').innerHTML = '<div class="loading-state">Aucun résultat pour le moment.</div>';
                    return;
                }

                let html = `
                    <table class="leaderboard-table">
                        <thead>
                            <tr>
                                <th style="width: 80px; text-align: center;">Rang</th>
                                <th>Système / OS</th>
                                <th>Processeur (CPU)</th>
                                <th>Score CPU</th>
                                <th>Score RAM</th>
                                <th>Score Disque</th>
                            </tr>
                        </thead>
                        <tbody>
                `;

                data.forEach((row, idx) => {
                    let rankClass = idx === 0 ? 'rank-1' : (idx === 1 ? 'rank-2' : (idx === 2 ? 'rank-3' : ''));
                    let rankIcon = idx === 0 ? '<i class="fa-solid fa-crown"></i>' : (idx + 1);

                    html += `
                        <tr class="${rankClass}">
                            <td style="text-align: center;">
                                <div class="rank-badge">${rankIcon}</div>
                            </td>
                            <td>
                                <div style="font-weight: 600;">Utilisateur #${row.id}</div>
                                <div class="spec-item"><i class="fa-brands fa-linux"></i> ${row.os_name}</div>
                            </td>
                            <td>
                                <div style="font-weight: 600;">${row.cpu_name}</div>
                                <div class="spec-item"><i class="fa-solid fa-microchip"></i> ${row.core_count} Cores • ${Math.round(row.memory_total_mb/1024)} GB RAM</div>
                            </td>
                            <td class="score-val">${row.cpu_score.toLocaleString()}</td>
                            <td class="score-val" style="color: #a855f7;">${row.ram_score.toLocaleString()} <span style="font-size:0.7em;">MB/s</span></td>
                            <td class="score-val" style="color: #3b82f6;">${row.disk_score.toLocaleString()} <span style="font-size:0.7em;">MB/s</span></td>
                        </tr>
                    `;
                });

                html += `</tbody></table>`;
                document.getElementById('leaderboard-container').innerHTML = html;
            } catch (e) {
                document.getElementById('leaderboard-container').innerHTML = '<div class="loading-state" style="color: #ef4444;"><i class="fa-solid fa-triangle-exclamation"></i> Impossible de charger le classement.</div>';
            }
        }

        // Init
        fetchLeaderboard();
    </script>
    </div>
</body>
</html>
