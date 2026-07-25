const API_URL = "https://nemdiag.nhtml.ynh.fr/api/telemetry";

// Mock data as fallback if API doesn't support GET or is offline
const MOCK_DATA = [
    { id: 1, cpu_score: 14200, gpu_score: 35000, cpu_name: "AMD Ryzen 9 7950X 16-Core Processor", os_name: "Linux Mint 21.2", core_count: 16 },
    { id: 2, cpu_score: 12500, gpu_score: 42000, cpu_name: "Intel Core i9-13900K", os_name: "Ubuntu 22.04", core_count: 24 },
    { id: 3, cpu_score: 9800, gpu_score: 12000, cpu_name: "AMD Ryzen 7 5800X3D", os_name: "Arch Linux", core_count: 8 },
    { id: 4, cpu_score: 8500, gpu_score: 25000, cpu_name: "Intel Core i7-12700K", os_name: "Fedora 38", core_count: 12 },
    { id: 5, cpu_score: 6400, gpu_score: 8000, cpu_name: "AMD Ryzen 5 5600X", os_name: "Debian 12", core_count: 6 },
    { id: 6, cpu_score: 4200, gpu_score: 3500, cpu_name: "Intel Core i5-10400F", os_name: "Pop!_OS 22.04", core_count: 6 },
    { id: 7, cpu_score: 2100, gpu_score: 0, cpu_name: "Intel Core i5-4570", os_name: "Linux Mint 21.2", core_count: 4 },
];

let globalData = [];

document.addEventListener("DOMContentLoaded", async () => {
    await fetchLeaderboard();
    
    // Search listener
    document.getElementById('search-input').addEventListener('input', (e) => {
        renderTable(e.target.value);
    });
});

async function fetchLeaderboard() {
    try {
        // Try to fetch from the actual API
        const response = await fetch(API_URL);
        if (response.ok) {
            const data = await response.json();
            globalData = data;
        } else {
            throw new Error("API returned non-200 status");
        }
    } catch (error) {
        console.warn("Could not fetch from API, falling back to mock data.", error);
        globalData = MOCK_DATA;
    }

    // Sort by CPU score descending
    globalData.sort((a, b) => b.cpu_score - a.cpu_score);
    
    updateStats();
    renderPodium();
    renderTable();
}

function updateStats() {
    const totalUsers = globalData.length;
    const avgCpu = Math.round(globalData.reduce((acc, curr) => acc + curr.cpu_score, 0) / totalUsers);
    const avgGpu = Math.round(globalData.reduce((acc, curr) => acc + curr.gpu_score, 0) / totalUsers);

    // Simple counter animation
    animateValue("stat-users", 0, totalUsers, 1000);
    animateValue("stat-avg-cpu", 0, avgCpu, 1000);
    animateValue("stat-avg-gpu", 0, avgGpu, 1000);
}

function renderPodium() {
    const container = document.getElementById("podium-container");
    container.innerHTML = "";
    
    if (globalData.length < 3) {
        container.innerHTML = "<p style='color:var(--text-muted)'>Pas assez de données pour afficher le podium.</p>";
        return;
    }

    // Top 3 (Reordered for visual layout: 2, 1, 3)
    const top3 = [globalData[1], globalData[0], globalData[2]];
    const classes = ["rank-2", "rank-1", "rank-3"];
    const icons = ["fa-medal", "fa-trophy", "fa-award"];

    top3.forEach((user, index) => {
        const item = document.createElement("div");
        item.className = `podium-item ${classes[index]}`;
        item.innerHTML = `
            <i class="fa-solid ${icons[index]} rank-icon"></i>
            <div class="podium-score">${user.cpu_score.toLocaleString()}</div>
            <div class="podium-cpu">${formatCpuName(user.cpu_name)}</div>
            <div style="margin-top:0.5rem; font-size:0.8rem; color:var(--text-muted)">
                <i class="fa-brands fa-linux"></i> ${user.os_name}
            </div>
        `;
        container.appendChild(item);
    });
}

function renderTable(searchFilter = "") {
    const tbody = document.getElementById("leaderboard-body");
    tbody.innerHTML = "";

    const filterLower = searchFilter.toLowerCase();
    const filtered = globalData.filter(u => 
        u.cpu_name.toLowerCase().includes(filterLower) || 
        u.os_name.toLowerCase().includes(filterLower)
    );

    if (filtered.length === 0) {
        tbody.innerHTML = `<tr><td colspan="6" style="text-align:center; padding: 3rem;">Aucun résultat trouvé.</td></tr>`;
        return;
    }

    filtered.forEach((user, index) => {
        // Find actual global rank
        const globalRank = globalData.findIndex(u => u === user) + 1;
        
        let rowClass = "";
        let rankDisplay = globalRank;
        
        if (globalRank === 1) { rowClass = "row-top1"; rankDisplay = '<i class="fa-solid fa-trophy"></i>'; }
        else if (globalRank === 2) { rowClass = "row-top2"; rankDisplay = '<i class="fa-solid fa-medal"></i>'; }
        else if (globalRank === 3) { rowClass = "row-top3"; rankDisplay = '<i class="fa-solid fa-award"></i>'; }

        const tr = document.createElement("tr");
        tr.className = rowClass;
        tr.innerHTML = `
            <td class="rank-col">#${rankDisplay}</td>
            <td style="font-weight:500;">${user.cpu_name}</td>
            <td>${user.core_count}</td>
            <td><span class="os-badge"><i class="fa-brands fa-linux"></i> ${user.os_name.split(' ')[0]}</span></td>
            <td class="score-col" style="color:var(--text-main);">${user.cpu_score.toLocaleString()}</td>
            <td class="score-col">${user.gpu_score > 0 ? user.gpu_score.toLocaleString() : '-'}</td>
        `;
        tbody.appendChild(tr);
    });
}

// Helpers
function formatCpuName(name) {
    // Simplify long CPU names for the podium display
    return name.replace("Processor", "").replace("16-Core", "").trim();
}

function animateValue(id, start, end, duration) {
    if (start === end) return;
    let obj = document.getElementById(id);
    if (!obj) return;
    
    let startTimestamp = null;
    const step = (timestamp) => {
        if (!startTimestamp) startTimestamp = timestamp;
        const progress = Math.min((timestamp - startTimestamp) / duration, 1);
        // Easing function (easeOutQuart)
        const easeProgress = 1 - Math.pow(1 - progress, 4);
        obj.innerHTML = Math.floor(easeProgress * (end - start) + start).toLocaleString();
        if (progress < 1) {
            window.requestAnimationFrame(step);
        }
    };
    window.requestAnimationFrame(step);
}
