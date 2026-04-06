// Storage keys
const API_KEY_STORAGE = 'b0_api_key'
const LEAD_ID_STORAGE = 'b0_lead_id'

export function getStoredApiKey(): string | null {
  return localStorage.getItem(API_KEY_STORAGE)
}

export function setStoredApiKey(key: string): void {
  localStorage.setItem(API_KEY_STORAGE, key)
}

export function clearStoredApiKey(): void {
  localStorage.removeItem(API_KEY_STORAGE)
}

export function getLeadId(): string {
  let id = localStorage.getItem(LEAD_ID_STORAGE)
  if (!id) {
    id = 'web-' + Math.random().toString(36).slice(2, 10)
    localStorage.setItem(LEAD_ID_STORAGE, id)
  }
  return id
}

// ----- Types -----

export interface Agent {
  name: string
  description: string
  instructions: string
  runtime: string
  status: string
  kind: string
  webhook_enabled: boolean
  webhook_secret: string | null
  created_at: string
}

export interface CronJob {
  id: string
  agent: string
  schedule: string
  task: string
  enabled: boolean
  last_run: string | null
}

export interface InboxMessage {
  id: string
  thread_id: string
  from: string
  to: string
  type: string
  content: string | null
  status: string
  created_at: string
}

export interface AgentThreadSummary {
  thread_id: string
  first_content: string
  latest_type: string
  latest_at: string
}

// ----- HTTP client -----

async function apiFetch<T>(path: string, options: RequestInit = {}): Promise<T> {
  const key = getStoredApiKey()
  const headers: Record<string, string> = { 'Content-Type': 'application/json' }
  if (key) headers['x-api-key'] = key

  const res = await fetch(path, {
    ...options,
    headers: { ...headers, ...(options.headers as Record<string, string> ?? {}) },
  })

  if (!res.ok) {
    const body = await res.json().catch(() => ({})) as Record<string, unknown>
    throw new Error((body.error as string) ?? `HTTP ${res.status}`)
  }

  return res.json() as Promise<T>
}

// ----- API functions -----

export async function getWorkspaces(): Promise<string[]> {
  const data = await apiFetch<{ workspaces: { name: string }[] }>('/workspaces')
  return data.workspaces.map(w => w.name)
}

export async function listAgents(workspace: string): Promise<Agent[]> {
  const data = await apiFetch<{ agents: Agent[] }>(`/workspaces/${workspace}/agents`)
  return data.agents
}

export async function listCronJobs(workspace: string): Promise<CronJob[]> {
  const data = await apiFetch<{ cron_jobs: CronJob[] }>(`/workspaces/${workspace}/cron`)
  return data.cron_jobs
}

export async function getAgentThreads(workspace: string, agent: string, limit = 20): Promise<AgentThreadSummary[]> {
  const data = await apiFetch<{ threads: AgentThreadSummary[] }>(
    `/workspaces/${workspace}/agents/${agent}/threads?limit=${limit}`,
  )
  return data.threads
}

export async function getThreadMessages(workspace: string, threadId: string): Promise<InboxMessage[]> {
  const data = await apiFetch<{ messages: InboxMessage[] }>(
    `/workspaces/${workspace}/threads/${threadId}`,
  )
  return data.messages
}

export async function runAgent(workspace: string, agent: string, task: string): Promise<string> {
  const threadId = 'thread-' + Math.random().toString(36).slice(2, 10)
  const leadId = getLeadId()
  await apiFetch(`/workspaces/${workspace}/agents/${agent}/inbox`, {
    method: 'POST',
    body: JSON.stringify({
      thread_id: threadId,
      from: leadId,
      type: 'request',
      content: task,
    }),
  })
  return threadId
}
