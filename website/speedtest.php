<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>NemDiag - Speedtest</title>
    <link href="https://fonts.googleapis.com/css2?family=Outfit:wght@300;400;600;800&family=JetBrains+Mono:wght@400;700&display=swap" rel="stylesheet">
    <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.4.0/css/all.min.css">
    <script type="text/javascript" src="speedtest.js"></script>
    <script type="text/javascript">
        function I(i){return document.getElementById(i);}
        
        var s=new Speedtest();
        s.setParameter("telemetry_level","none"); 
        
        function format(d){
            d=Number(d);
            if(d<10) return d.toFixed(2);
            if(d<100) return d.toFixed(1);
            return d.toFixed(0);
        }
        
        var meterBk=/Trident.*rv:(\d+\.\d+)/i.test(navigator.userAgent)?"#0f172a":"#0f172a";
        var dlColor="#00e676", ulColor="#38bdf8", pingColor="#f59e0b", jitColor="#a855f7";
        var progColor=meterBk;

        function drawMeter(c,amount,bk,fg,progress,prog){
            var ctx=c.getContext("2d");
            var dp=window.devicePixelRatio||1;
            var cw=c.clientWidth*dp, ch=c.clientHeight*dp;
            var sizScale=ch*0.0115;
            if(c.width==cw&&c.height==ch){
                ctx.clearRect(0,0,cw,ch);
            }else{
                c.width=cw;
                c.height=ch;
            }
            ctx.beginPath();
            ctx.strokeStyle=bk;
            ctx.lineWidth=12*sizScale;
            ctx.arc(c.width/2,c.height-58*sizScale,c.height/1.8-ctx.lineWidth,-Math.PI*1.1,Math.PI*0.1);
            ctx.stroke();
            ctx.beginPath();
            ctx.strokeStyle=fg;
            ctx.lineWidth=12*sizScale;
            ctx.arc(c.width/2,c.height-58*sizScale,c.height/1.8-ctx.lineWidth,-Math.PI*1.1,amount*Math.PI*1.2-Math.PI*1.1);
            ctx.stroke();
            if(typeof progress !== "undefined"){
                ctx.fillStyle=prog;
                ctx.fillRect(c.width*0.3,c.height-16*sizScale,c.width*0.4*progress,4*sizScale);
            }
        }
        function mbpsToAmount(s){
            return 1-(1/(Math.pow(1.3,Math.sqrt(s))));
        }

        s.onupdate=function(data){
            var status=data.testState;
            if(status===1&&data.dlStatus===0){
                I("pingText").textContent=format(data.pingStatus);
                I("jitText").textContent=format(data.jitterStatus);
            }
            if(status===2){
                I("dlText").textContent=(data.dlStatus==0)?"...":format(data.dlStatus);
                drawMeter(I("dlMeter"),mbpsToAmount(Number(data.dlStatus*(status==2?1:0))),meterBk,dlColor,Number(data.dlProgress),progColor);
            }
            if(status===3){
                I("ulText").textContent=(data.ulStatus==0)?"...":format(data.ulStatus);
                drawMeter(I("ulMeter"),mbpsToAmount(Number(data.ulStatus*(status==3?1:0))),meterBk,ulColor,Number(data.ulProgress),progColor);
            }
            if(status===4){
                // Finished
                drawMeter(I("dlMeter"),mbpsToAmount(Number(data.dlStatus)),meterBk,dlColor,0,progColor);
                drawMeter(I("ulMeter"),mbpsToAmount(Number(data.ulStatus)),meterBk,ulColor,0,progColor);
                I("startStopBtn").textContent="Relancer";
                I("startStopBtn").style.backgroundColor = "var(--primary)";
                I("startStopBtn").style.color = "var(--bg-dark)";
            }
            if(status===5){
                // Aborted
                I("startStopBtn").textContent="Démarrer";
            }
        };

        function startStop(){
            if(s.getState()==3){
                s.abort();
                I("startStopBtn").textContent="Démarrer";
                I("startStopBtn").style.backgroundColor = "var(--primary)";
                I("startStopBtn").style.color = "var(--bg-dark)";
            }else{
                s.start();
                I("startStopBtn").textContent="Arrêter";
                I("startStopBtn").style.backgroundColor = "rgba(255,0,0,0.2)";
                I("startStopBtn").style.color = "#ff5252";
                drawMeter(I("dlMeter"),0,meterBk,dlColor,0,progColor);
                drawMeter(I("ulMeter"),0,meterBk,ulColor,0,progColor);
                I("dlText").textContent="";
                I("ulText").textContent="";
                I("pingText").textContent="";
                I("jitText").textContent="";
            }
        }
        
        // init meters
        window.onload=function(){
            drawMeter(I("dlMeter"),0,meterBk,dlColor,0,progColor);
            drawMeter(I("ulMeter"),0,meterBk,ulColor,0,progColor);
        };
    </script>
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

        .speedtest-container {
            margin: 4rem auto;
            max-width: 800px;
            background: var(--card-bg);
            border: 1px solid var(--border);
            border-radius: 16px;
            padding: 3rem;
            text-align: center;
            box-shadow: 0 20px 50px rgba(0,0,0,0.5);
        }

        .meters { display: flex; justify-content: center; gap: 2rem; margin-bottom: 2rem; flex-wrap: wrap; }
        .meter {
            width: 240px; height: 240px;
            position: relative;
        }
        .meter canvas {
            width: 100%; height: 100%;
        }
        .meter-text {
            position: absolute;
            top: 50%; left: 50%; transform: translate(-50%, -10%);
            text-align: center;
        }
        .meter-val { font-family: 'JetBrains Mono', monospace; font-size: 2.5rem; font-weight: 700; }
        .meter-unit { font-size: 0.9rem; color: var(--text-dim); font-weight: 600; letter-spacing: 1px; }

        .ping-jit { display: flex; justify-content: center; gap: 3rem; margin-bottom: 3rem; }
        .pj-box { text-align: center; }
        .pj-val { font-family: 'JetBrains Mono', monospace; font-size: 1.8rem; font-weight: 700; }
        .pj-label { font-size: 0.8rem; color: var(--text-dim); text-transform: uppercase; letter-spacing: 1px; }

        #startStopBtn {
            background: var(--primary);
            color: var(--bg-dark);
            border: none;
            padding: 1rem 3rem;
            font-size: 1.2rem;
            font-weight: 700;
            border-radius: 30px;
            cursor: pointer;
            transition: all 0.3s;
            text-transform: uppercase;
            letter-spacing: 1px;
            box-shadow: 0 0 20px var(--primary-glow);
        }
        #startStopBtn:hover { transform: scale(1.05); }

    </style>
</head>
<body>
    <div class="container">
        <nav>
            <a href="index.php" class="logo"><i class="fa-solid fa-microchip"></i> NemDiag</a>
            <div class="nav-links">
                <a href="index.php">Accueil</a>
                <a href="speedtest.php" class="active">Speedtest</a>
                <a href="leaderboard.php">Podium</a>
            </div>
        </nav>

        <div class="speedtest-container">
            <h2 style="margin-bottom: 2rem; font-size: 2rem; font-weight: 800;">Test de Débit <span style="color: var(--primary);">Local</span></h2>
            
            <div class="meters">
                <div class="meter">
                    <canvas id="dlMeter"></canvas>
                    <div class="meter-text">
                        <div class="meter-val" id="dlText" style="color: var(--primary);"></div>
                        <div class="meter-unit">Mbps Descendant</div>
                    </div>
                </div>
                <div class="meter">
                    <canvas id="ulMeter"></canvas>
                    <div class="meter-text">
                        <div class="meter-val" id="ulText" style="color: var(--accent);"></div>
                        <div class="meter-unit">Mbps Montant</div>
                    </div>
                </div>
            </div>

            <div class="ping-jit">
                <div class="pj-box">
                    <div class="pj-label">Ping</div>
                    <div class="pj-val" id="pingText" style="color: #f59e0b;">-</div>
                    <div class="pj-label" style="text-transform:none;">ms</div>
                </div>
                <div class="pj-box">
                    <div class="pj-label">Gigue</div>
                    <div class="pj-val" id="jitText" style="color: #a855f7;">-</div>
                    <div class="pj-label" style="text-transform:none;">ms</div>
                </div>
            </div>

            <button id="startStopBtn" onclick="startStop()">Démarrer</button>
        </div>
    </div>
</body>
</html>