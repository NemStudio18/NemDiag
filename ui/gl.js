const vertexShaderSource = `
    attribute vec2 position;
    void main() {
        gl_Position = vec4(position, 0.0, 1.0);
    }
`;

const fragmentShaderSource = `
    precision highp float;
    uniform vec2 u_resolution;
    uniform float u_time;
    
    // Fractal / Raymarching math to stress GPU
    void main() {
        vec2 uv = gl_FragCoord.xy / u_resolution.xy;
        uv = uv * 2.0 - 1.0;
        uv.x *= u_resolution.x / u_resolution.y;
        
        float zoom = 1.0 + 0.5 * sin(u_time * 0.5);
        vec2 c = uv * zoom;
        c += vec2(-0.5, 0.0); // Offset to center Mandelbrot
        
        vec2 z = vec2(0.0);
        float iter = 0.0;
        const float max_iter = 250.0; // High iteration count for stress
        
        for(float i = 0.0; i < 250.0; i++) {
            if(dot(z, z) > 4.0) break;
            z = vec2(z.x*z.x - z.y*z.y, 2.0*z.x*z.y) + c;
            iter++;
        }
        
        float color = iter / max_iter;
        vec3 rgb = vec3(color * sin(u_time), color * 0.2, color * cos(u_time));
        
        gl_FragColor = vec4(rgb, 1.0);
    }
`;

let glContext = null;
let glProgram = null;
let animationFrameId = null;
let lastFrameTime = performance.now();
let frameCount = 0;
let fpsEl = null;

function initWebGL(canvasId) {
    const canvas = document.getElementById(canvasId);
    if (!canvas) return false;
    
    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;
    
    glContext = canvas.getContext('webgl') || canvas.getContext('experimental-webgl');
    if (!glContext) {
        console.error("WebGL not supported");
        return false;
    }
    
    const gl = glContext;
    
    // Compile shaders
    const vs = gl.createShader(gl.VERTEX_SHADER);
    gl.shaderSource(vs, vertexShaderSource);
    gl.compileShader(vs);
    
    const fs = gl.createShader(gl.FRAGMENT_SHADER);
    gl.shaderSource(fs, fragmentShaderSource);
    gl.compileShader(fs);
    
    // Create program
    glProgram = gl.createProgram();
    gl.attachShader(glProgram, vs);
    gl.attachShader(glProgram, fs);
    gl.linkProgram(glProgram);
    gl.useProgram(glProgram);
    
    // Set up geometry (full screen quad)
    const vertices = new Float32Array([
        -1, -1,
         1, -1,
        -1,  1,
         1, -1,
         1,  1,
        -1,  1,
    ]);
    
    const vbo = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
    gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STATIC_DRAW);
    
    const posAttrib = gl.getAttribLocation(glProgram, "position");
    gl.enableVertexAttribArray(posAttrib);
    gl.vertexAttribPointer(posAttrib, 2, gl.FLOAT, false, 0, 0);
    
    return true;
}

function startWebGLStress() {
    const canvas = document.getElementById('webgl-canvas');
    if (!canvas) return;
    
    canvas.style.display = 'block';
    
    if (!glContext && !initWebGL('webgl-canvas')) return;
    
    const gl = glContext;
    const resUniform = gl.getUniformLocation(glProgram, "u_resolution");
    const timeUniform = gl.getUniformLocation(glProgram, "u_time");
    
    fpsEl = document.getElementById('webgl-fps');
    if (fpsEl) fpsEl.parentElement.style.display = 'block';
    
    let startTime = performance.now();
    lastFrameTime = startTime;
    frameCount = 0;
    
    function render(now) {
        // Calculate FPS
        frameCount++;
        if (now - lastFrameTime >= 1000) {
            if (fpsEl) fpsEl.innerText = frameCount;
            frameCount = 0;
            lastFrameTime = now;
        }
        
        gl.viewport(0, 0, gl.canvas.width, gl.canvas.height);
        gl.uniform2f(resUniform, gl.canvas.width, gl.canvas.height);
        gl.uniform1f(timeUniform, (now - startTime) / 1000.0);
        
        gl.drawArrays(gl.TRIANGLES, 0, 6);
        
        animationFrameId = requestAnimationFrame(render);
    }
    
    animationFrameId = requestAnimationFrame(render);
}

function stopWebGLStress() {
    if (animationFrameId) {
        cancelAnimationFrame(animationFrameId);
        animationFrameId = null;
    }
    const canvas = document.getElementById('webgl-canvas');
    if (canvas) canvas.style.display = 'none';
    if (fpsEl) fpsEl.parentElement.style.display = 'none';
}
