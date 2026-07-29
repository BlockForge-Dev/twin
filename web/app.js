// State storage
let allDevices = [];
let allStateMap = {};
let allAlerts = [];
let activeTab = 'cylinders';

// Initialize on page load
document.addEventListener('DOMContentLoaded', () => {
  loadAllData();
  // Live polling every 5 seconds
  setInterval(loadAllData, 5000);
});

async function loadAllData() {
  await Promise.all([
    loadDevicesAndStates(),
    loadAlerts()
  ]);
  updateSummaryMetrics();
}

async function manualRefresh() {
  const liveText = document.getElementById('live-text');
  if (liveText) liveText.innerText = 'Refreshing...';
  await loadAllData();
  if (liveText) liveText.innerText = 'Live (5s)';
}

// ── Tab Navigation ───────────────────────────────────────────────────────────

function switchTab(tabName) {
  activeTab = tabName;
  document.querySelectorAll('.nav-btn').forEach(btn => btn.classList.remove('active'));
  document.querySelectorAll('.tab-view').forEach(view => view.classList.remove('active'));

  const activeNav = document.getElementById(`nav-${tabName}`);
  const activeView = document.getElementById(`view-${tabName}`);
  if (activeNav) activeNav.classList.add('active');
  if (activeView) activeView.classList.add('active');

  if (tabName === 'alerts') renderAlertsFeed();
}

// ── API Fetching ──────────────────────────────────────────────────────────────

async function loadDevicesAndStates() {
  try {
    const res = await fetch('/api/v1/devices');
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    allDevices = await res.json();

    // Fetch derived state for each device in parallel
    const statePromises = allDevices.map(dev =>
      fetch(`/api/v1/devices/${dev.device_id}/state`)
        .then(r => r.ok ? r.json() : null)
        .catch(() => null)
    );

    const states = await Promise.all(statePromises);
    allStateMap = {};
    allDevices.forEach((dev, idx) => {
      allStateMap[dev.device_id] = states[idx];
    });

    renderCylinderGrid();
  } catch (err) {
    console.error('Error fetching devices:', err);
    const grid = document.getElementById('cylinder-grid');
    if (grid && allDevices.length === 0) {
      grid.innerHTML = `<div class="loading-state"><p class="text-rose">Failed to connect to backend ingest server.</p></div>`;
    }
  }
}

async function loadAlerts() {
  try {
    const res = await fetch('/api/v1/alerts');
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    allAlerts = await res.json();

    const badge = document.getElementById('badge-alert-count');
    if (badge) badge.innerText = allAlerts.filter(a => !a.acknowledged_at).length;

    if (activeTab === 'alerts') renderAlertsFeed();
  } catch (err) {
    console.error('Error fetching alerts:', err);
  }
}

// ── Rendering ────────────────────────────────────────────────────────────────

function updateSummaryMetrics() {
  document.getElementById('badge-total-count').innerText = allDevices.length;
  document.getElementById('stat-total').innerText = allDevices.length;

  let normal = 0, low = 0, critical = 0;
  Object.values(allStateMap).forEach(state => {
    if (!state) return;
    if (state.status === 'normal') normal++;
    else if (state.status === 'low') low++;
    else if (state.status === 'critical') critical++;
  });

  document.getElementById('stat-normal').innerText = normal;
  document.getElementById('stat-low').innerText = low;
  document.getElementById('stat-critical').innerText = critical;
}

function filterCylinders() {
  renderCylinderGrid();
}

function renderCylinderGrid() {
  const grid = document.getElementById('cylinder-grid');
  if (!grid) return;

  const search = document.getElementById('search-input')?.value.toLowerCase() || '';

  const filtered = allDevices.filter(dev =>
    dev.device_id.toLowerCase().includes(search) ||
    (dev.site_id && dev.site_id.toLowerCase().includes(search))
  );

  if (filtered.length === 0) {
    grid.innerHTML = `<div class="loading-state"><p>No cylinders found.</p></div>`;
    return;
  }

  grid.innerHTML = filtered.map(dev => {
    const state = allStateMap[dev.device_id];
    const remainingGrams = state?.remaining_grams ?? null;
    const remainingKg = remainingGrams !== null ? (remainingGrams / 1000.0).toFixed(2) : '--';
    const status = state?.status || dev.status || 'unknown';
    const lastSeen = state?.last_seen_at ? formatRelativeTime(state.last_seen_at) : 'Never';

    // Default fill 12.5kg for pct calculation
    const fillGrams = 12500;
    const pct = remainingGrams !== null ? Math.min(100, Math.max(0, Math.round((remainingGrams / fillGrams) * 100))) : 0;

    let fillClass = '';
    if (status === 'low') fillClass = 'low';
    if (status === 'critical') fillClass = 'critical';

    return `
      <div class="cylinder-card" onclick="openDetailModal('${dev.device_id}')">
        <div class="card-top">
          <div>
            <div class="card-device-id">${escapeHtml(dev.device_id)}</div>
            <div class="card-site-id">${escapeHtml(dev.site_id || 'Unassigned Site')}</div>
          </div>
          <span class="badge ${status}">
            ${status === 'critical' ? '⚡ ' : ''}${status}
          </span>
        </div>

        <div class="card-number-display">
          <span class="big-number">${remainingKg}</span>
          <span class="unit">kg remaining</span>
        </div>

        <div class="progress-bar-bg">
          <div class="progress-bar-fill ${fillClass}" style="width: ${pct}%;"></div>
        </div>

        <div class="card-bottom">
          <span>Last seen: ${lastSeen}</span>
          <span class="btn-card-action">Manage & Refill &rarr;</span>
        </div>
      </div>
    `;
  }).join('');
}

function renderAlertsFeed() {
  const container = document.getElementById('alerts-list');
  if (!container) return;

  if (allAlerts.length === 0) {
    container.innerHTML = `<div class="loading-state"><p>No alert events recorded.</p></div>`;
    return;
  }

  container.innerHTML = allAlerts.map(alert => {
    const isAck = !!alert.acknowledged_at;
    const timeStr = formatRelativeTime(alert.triggered_at);

    return `
      <div class="alert-item">
        <div class="alert-info">
          <div class="alert-icon">🔔</div>
          <div class="alert-body">
            <h4>${escapeHtml(alert.device_id)}: ${alert.state_from.toUpperCase()} &rarr; ${alert.state_to.toUpperCase()}</h4>
            <p>${escapeHtml(alert.message || '')}</p>
            <div class="alert-time">Triggered: ${timeStr}</div>
          </div>
        </div>
        <div>
          ${isAck
            ? `<button class="btn-ack ack-done" disabled>&check; Acknowledged</button>`
            : `<button class="btn-ack" onclick="handleAcknowledgeAlert('${alert.id}')">Acknowledge</button>`
          }
        </div>
      </div>
    `;
  }).join('');
}

// ── Detail Modal ─────────────────────────────────────────────────────────────

async function openDetailModal(deviceId) {
  const dev = allDevices.find(d => d.device_id === deviceId);
  const state = allStateMap[deviceId];

  document.getElementById('modal-device-id').innerText = deviceId;
  document.getElementById('modal-site-name').innerText = dev?.site_id || 'Unassigned Site';
  document.getElementById('refill-device-id').value = deviceId;

  const remainingGrams = state?.remaining_grams ?? null;
  const remainingKg = remainingGrams !== null ? (remainingGrams / 1000.0).toFixed(2) : '--';
  const status = state?.status || dev?.status || 'unknown';

  document.getElementById('modal-remaining-kg').innerText = remainingKg;
  const badge = document.getElementById('modal-status-badge');
  badge.className = `badge ${status}`;
  badge.innerText = status;

  document.getElementById('modal-last-seen').innerText = state?.last_seen_at
    ? `Last seen: ${formatRelativeTime(state.last_seen_at)}`
    : 'Last seen: --';

  const fillGrams = 12500;
  const pct = remainingGrams !== null ? Math.min(100, Math.max(0, Math.round((remainingGrams / fillGrams) * 100))) : 0;
  const bar = document.getElementById('modal-progress-fill');
  bar.style.width = `${pct}%`;
  bar.className = `progress-bar-fill ${status === 'low' ? 'low' : status === 'critical' ? 'critical' : ''}`;

  // Fetch Refills Audit History & Device Alerts in parallel
  loadModalRefillsAndAlerts(deviceId);

  document.getElementById('detail-modal').classList.add('open');
}

async function loadModalRefillsAndAlerts(deviceId) {
  const refillContainer = document.getElementById('modal-refill-history');
  const alertContainer = document.getElementById('modal-device-alerts');

  refillContainer.innerHTML = '<p class="empty-text">Loading...</p>';
  alertContainer.innerHTML = '<p class="empty-text">Loading...</p>';

  try {
    const [refillRes, alertRes] = await Promise.all([
      fetch(`/api/v1/devices/${deviceId}/refills`),
      fetch(`/api/v1/alerts?device_id=${deviceId}`)
    ]);

    if (refillRes.ok) {
      const refills = await refillRes.json();
      if (refills.length === 0) {
        refillContainer.innerHTML = '<p class="empty-text">No refills recorded yet.</p>';
      } else {
        refillContainer.innerHTML = refills.map(r => `
          <div class="history-item">
            <div>
              <strong>${(r.fill_amount_grams / 1000.0).toFixed(1)} kg</strong>
              ${r.edited_by ? `<span style="color:var(--text-muted);"> (by ${escapeHtml(r.edited_by)})</span>` : ''}
            </div>
            <div style="color:var(--text-dim);">${formatRelativeTime(r.refill_date)}</div>
          </div>
        `).join('');
      }
    }

    if (alertRes.ok) {
      const alerts = await alertRes.json();
      if (alerts.length === 0) {
        alertContainer.innerHTML = '<p class="empty-text">No alerts recorded for this device.</p>';
      } else {
        alertContainer.innerHTML = alerts.map(a => `
          <div class="history-item">
            <div>
              <span class="badge ${a.state_to}">${a.state_from} &rarr; ${a.state_to}</span>
            </div>
            <div style="color:var(--text-dim);">${formatRelativeTime(a.triggered_at)}</div>
          </div>
        `).join('');
      }
    }

  } catch (err) {
    console.error('Error loading modal details:', err);
  }
}

function closeDetailModal() {
  document.getElementById('detail-modal').classList.remove('open');
}

function closeModalOnBackdrop(e) {
  if (e.target.classList.contains('modal-backdrop')) {
    closeDetailModal();
  }
}

// ── Action Handlers ──────────────────────────────────────────────────────────

async function handleRecordRefill(event) {
  event.preventDefault();
  const deviceId = document.getElementById('refill-device-id').value;
  const kg = parseFloat(document.getElementById('refill-kg').value);
  const name = document.getElementById('refill-name').value;
  const operator = document.getElementById('refill-operator').value;
  const notes = document.getElementById('refill-notes').value;
  const statusMsg = document.getElementById('refill-status-msg');

  try {
    const res = await fetch(`/api/v1/devices/${deviceId}/refill`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        fill_amount_kg: kg,
        cylinder_name: name || null,
        edited_by: operator || null,
        notes: notes || null
      })
    });

    if (res.ok) {
      statusMsg.style.color = 'var(--emerald-500)';
      statusMsg.innerText = 'Refill submitted successfully! Gas level reset.';
      await loadDevicesAndStates();
      openDetailModal(deviceId); // refresh modal view
      setTimeout(() => { statusMsg.innerText = ''; }, 3000);
    } else {
      const err = await res.json();
      statusMsg.style.color = 'var(--rose-500)';
      statusMsg.innerText = `Error: ${err.error || 'Failed to submit refill'}`;
    }
  } catch (err) {
    statusMsg.style.color = 'var(--rose-500)';
    statusMsg.innerText = 'Network error while submitting refill.';
  }
}

async function handleRegisterDevice(event) {
  event.preventDefault();
  const deviceId = document.getElementById('reg-device-id').value;
  const model = document.getElementById('reg-model').value;
  const firmware = document.getElementById('reg-firmware').value;
  const statusMsg = document.getElementById('reg-status-message');

  try {
    const res = await fetch('/api/v1/devices', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        device_id: deviceId,
        model: model || null,
        firmware_version: firmware || null
      })
    });

    if (res.ok) {
      statusMsg.style.color = 'var(--emerald-500)';
      statusMsg.innerText = `Device '${deviceId}' registered successfully!`;
      document.getElementById('form-register-device').reset();
      await loadDevicesAndStates();
      setTimeout(() => {
        statusMsg.innerText = '';
        switchTab('cylinders');
      }, 1500);
    } else {
      const err = await res.json();
      statusMsg.style.color = 'var(--rose-500)';
      statusMsg.innerText = `Error: ${err.error || 'Failed to register device'}`;
    }
  } catch (err) {
    statusMsg.style.color = 'var(--rose-500)';
    statusMsg.innerText = 'Network error while registering device.';
  }
}

async function handleAcknowledgeAlert(alertId) {
  try {
    const res = await fetch(`/api/v1/alerts/${alertId}/acknowledge`, {
      method: 'POST'
    });
    if (res.ok) {
      await loadAlerts();
    }
  } catch (err) {
    console.error('Error acknowledging alert:', err);
  }
}

// ── Helper Formatting ────────────────────────────────────────────────────────

function formatRelativeTime(isoString) {
  if (!isoString) return '--';
  const date = new Date(isoString);
  const now = new Date();
  const diffSecs = Math.floor((now - date) / 1000);

  if (diffSecs < 5) return 'Just now';
  if (diffSecs < 60) return `${diffSecs}s ago`;
  if (diffSecs < 3600) return `${Math.floor(diffSecs / 60)}m ago`;
  if (diffSecs < 86400) return `${Math.floor(diffSecs / 3600)}h ago`;
  return date.toLocaleDateString();
}

function escapeHtml(str) {
  if (!str) return '';
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}
