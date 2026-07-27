<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>NemDiag - Diagnostic Système Linux</title>
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
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
        html { scroll-behavior: smooth; }

        body {
            font-family: 'Outfit', sans-serif;
            background-color: var(--bg-dark);
            background-image: 
                radial-gradient(circle at 0% 0%, rgba(0, 230, 118, 0.08) 0%, transparent 40%),
                radial-gradient(circle at 100% 100%, rgba(56, 189, 248, 0.08) 0%, transparent 40%),
                linear-gradient(to bottom, #020617, #0f172a);
            background-attachment: fixed;
            color: var(--text);
            line-height: 1.6;
            overflow-x: hidden;
            min-height: 100vh;
            display: flex;
            flex-direction: column;
        }

        .container { max-width: 1200px; margin: 0 auto; padding: 0 1.5rem; width: 100%; }

        nav {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 2rem 0;
            position: sticky;
            top: 0;
            z-index: 1000;
            backdrop-filter: blur(10px);
            border-bottom: 1px solid var(--border);
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
        .nav-links a:hover, .nav-links a.active {
            color: var(--primary);
            text-shadow: 0 0 10px var(--primary-glow);
        }

        .nav-actions .btn-download {
            background: rgba(0, 230, 118, 0.1);
            color: var(--primary);
            border: 1px solid var(--primary);
            padding: 0.5rem 1.2rem;
            border-radius: 20px;
            font-size: 0.85rem;
            font-weight: 600;
            text-decoration: none;
            transition: all 0.3s ease;
        }
        .nav-actions .btn-download:hover {
            background: var(--primary);
            color: var(--bg-dark);
            box-shadow: 0 0 20px var(--primary-glow);
        }

        .hero { padding: 8rem 0 6rem; text-align: center; position: relative; flex: 1; }
        .hero::before {
            content: ''; position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%);
            width: 600px; height: 600px;
            background: radial-gradient(circle, rgba(0,230,118,0.05) 0%, transparent 70%);
            z-index: -1; pointer-events: none;
        }

        .hero-badge {
            display: inline-block;
            background: rgba(56, 189, 248, 0.1);
            color: var(--accent);
            border: 1px solid rgba(56, 189, 248, 0.2);
            padding: 0.4rem 1rem;
            border-radius: 20px;
            font-size: 0.8rem;
            font-weight: 600;
            margin-bottom: 2rem;
            letter-spacing: 1px;
        }

        .hero h1 { font-size: 4.5rem; font-weight: 900; line-height: 1.1; margin-bottom: 1.5rem; letter-spacing: -2px; }
        .hero h1 span { background: linear-gradient(to right, var(--text), var(--primary)); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
        .hero p { font-size: 1.25rem; color: var(--text-dim); max-width: 600px; margin: 0 auto 3rem; }

        .hero-cta { display: flex; justify-content: center; gap: 1.5rem; }
        .btn-primary {
            background: var(--primary); color: var(--bg-dark); padding: 1rem 2rem; border-radius: 8px; font-weight: 700; font-size: 1.1rem; text-decoration: none; display: flex; align-items: center; gap: 10px; transition: all 0.3s; border: 1px solid var(--primary);
        }
        .btn-primary:hover { transform: translateY(-2px); box-shadow: 0 10px 25px var(--primary-glow); }
        
        .btn-secondary {
            background: var(--card-bg); color: var(--text); padding: 1rem 2rem; border-radius: 8px; font-weight: 600; font-size: 1.1rem; text-decoration: none; display: flex; align-items: center; gap: 10px; border: 1px solid var(--border); transition: all 0.3s;
        }
        .btn-secondary:hover { border-color: var(--text-dim); background: rgba(255,255,255,0.05); }
    </style>
</head>
<body>
    <div class="container">
        <nav>
            <a href="index.php" class="logo"><i class="fa-solid fa-microchip"></i> NemDiag</a>
            <div class="nav-links">
                <a href="index.php" class="active">Accueil</a>
                <a href="speedtest.php">Speedtest</a>
                <a href="leaderboard.php">Podium</a>
            </div>
            <div class="nav-actions">
                <a href="https://github.com/NemStudio18/NemDiag/releases/latest" class="btn-download" target="_blank"><i class="fa-brands fa-linux"></i> Télécharger</a>
            </div>
        </nav>

        <section class="hero">
            <div class="hero-badge">VERSION 0.2.0 (RUST)</div>
            <h1>Diagnostiquez votre PC<br><span>à la vitesse de l'éclair.</span></h1>
            <p>NemDiag est un utilitaire open-source pour Linux qui analyse, stresse et compare les performances de votre matériel de manière entièrement sécurisée et transparente.</p>
            <div class="hero-cta">
                <a href="speedtest.php" class="btn-primary"><i class="fa-solid fa-gauge-high"></i> Tester ma connexion</a>
                <a href="leaderboard.php" class="btn-secondary"><i class="fa-solid fa-trophy"></i> Voir le Podium</a>
            </div>
        </section>
    </div>
</body>
</html>
