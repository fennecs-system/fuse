'use strict';
const $ = id => document.getElementById(id);
async function api(path, method = 'GET', body) {
  const result = await fetch(path, {method, body, headers: {'Authorization': `Bearer ${$('token').value.trim()}`}, cache: 'no-store'});
  const text = await result.text();
  if (!result.ok) throw new Error(text);
  return text;
}
$('run').onclick = async () => {
  if (!$('prompt').value.trim()) return;
  $('run').disabled = true;
  try { $('status').textContent = await api('/api/run', 'POST', $('prompt').value); }
  catch (e) { $('status').textContent = e.message; }
  finally { $('run').disabled = false; }
};
$('cancel').onclick = async () => {
  try { $('status').textContent = await api('/api/cancel', 'POST', ''); }
  catch (e) { $('status').textContent = e.message; }
};
async function poll() {
  if ($('token').value.trim()) {
    try {
      $('status').textContent = await api('/api/status');
      $('log').textContent = await api('/api/log');
    } catch (e) { $('status').textContent = e.message; }
  }
  setTimeout(poll, 1500);
}
poll();
