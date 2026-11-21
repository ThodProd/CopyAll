'use strict';

document.documentElement.dataset.theme = new URLSearchParams(location.search).get('theme') || 'light';

const byId = id => document.getElementById(id);
const actions = {
  'context-install': ['context.install', 'Добавляем пункт в контекстное меню…'],
  'context-remove': ['context.remove', 'Удаляем пункт из контекстного меню…'],
  'startup-install': ['startup.install', 'Включаем автозапуск…'],
  'startup-remove': ['startup.remove', 'Отключаем автозапуск…'],
};

function render(state) {
  byId('context-state').textContent = state.modernEnabled
    ? '«Копировать полный путь» добавлен в меню Проводника.'
    : state.contextEnabled
      ? '«Копировать полный путь» доступен в классическом меню Проводника.'
      : 'Пункт меню отключён.';
  byId('startup-state').textContent = state.startupEnabled
    ? 'CopyAll запускается при входе в Windows и проверяет регистрацию меню.'
    : 'Автозапуск отключён.';
  byId('context-install').disabled = state.contextReady;
  byId('context-remove').disabled = !state.contextEnabled;
  byId('startup-install').disabled = state.startupEnabled;
  byId('startup-remove').disabled = !state.startupEnabled;
}

async function runJob(method) {
  await window.thodHub.call('worker.' + method);
  for (;;) {
    const job = await window.thodHub.call('worker.settings.poll');
    if (!job.running) return job.state;
    await new Promise(resolve => setTimeout(resolve, 350));
  }
}

async function update(method, message) {
  const status = byId('status');
  for (const id of Object.keys(actions)) byId(id).disabled = true;
  status.textContent = message;
  try {
    render(await runJob(method));
    status.textContent = '';
  } catch (error) {
    if (method !== 'settings.status') {
      try { render(await runJob('settings.status')); }
      catch { for (const id of Object.keys(actions)) byId(id).disabled = false; }
    } else {
      for (const id of Object.keys(actions)) byId(id).disabled = false;
    }
    status.textContent = error.message || String(error);
  }
}

for (const [id, [method, message]] of Object.entries(actions)) {
  byId(id).addEventListener('click', () => void update(method, message));
}
void update('settings.status', 'Проверяем настройки…');
