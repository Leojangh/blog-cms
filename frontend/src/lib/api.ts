const API = import.meta.env.PUBLIC_API_URL || 'https://api.genlz.com';

export async function api(path: string, opts: RequestInit = {}) {
  return fetch(`${API}${path}`, { credentials: 'include', ...opts });
}

export async function apiJson(path: string, opts: RequestInit = {}) {
  const res = await api(path, opts);
  if (!res.ok) throw { status: res.status, body: await res.json().catch(() => null) };
  return res.json();
}
