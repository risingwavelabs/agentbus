<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { init, getWorkspaces, listAgents, listCronJobs, type Agent, type CronJob } from './api'
import AgentList from './components/AgentList.vue'
import AgentDetail from './components/AgentDetail.vue'

const ready = ref(false)
const initError = ref('')

const workspace = ref('')
const agents = ref<Agent[]>([])
const cronJobs = ref<CronJob[]>([])
const selectedAgent = ref<Agent | null>(null)

async function loadAgents() {
  const [a, c] = await Promise.all([listAgents(workspace.value), listCronJobs(workspace.value)])
  agents.value = a
  cronJobs.value = c
  if (selectedAgent.value) {
    selectedAgent.value = a.find(ag => ag.name === selectedAgent.value!.name) ?? null
  }
}

function selectAgent(agent: Agent) { selectedAgent.value = agent }

onMounted(async () => {
  try {
    await init()
    const workspaces = await getWorkspaces()
    if (!workspaces.length) throw new Error('No workspaces found.')
    workspace.value = workspaces[0]
    await loadAgents()
    ready.value = true
  } catch (e: any) {
    initError.value = e.message
  }
})
</script>

<template>
  <div v-if="initError" class="auth-page">
    <div class="auth-card">
      <h1>Box<span style="color:var(--accent)">0</span></h1>
      <p style="color:var(--error)">{{ initError }}</p>
      <p style="margin-top:8px">Make sure the Box0 server is running:</p>
      <pre style="margin-top:12px;background:var(--surface-2);padding:12px;border-radius:6px;font-size:13px">b0 server</pre>
    </div>
  </div>

  <div v-else-if="!ready" class="auth-page">
    <div style="color:var(--muted)">Connecting...</div>
  </div>

  <div v-else class="layout">
    <aside class="sidebar">
      <div class="sidebar-header">
        <span class="logo">Box<span>0</span></span>
      </div>
      <AgentList
        :agents="agents"
        :cron-jobs="cronJobs"
        :selected="selectedAgent?.name ?? ''"
        @select="selectAgent"
      />
    </aside>

    <main class="main">
      <AgentDetail v-if="selectedAgent" :agent="selectedAgent" :workspace="workspace" />
      <div v-else style="flex:1;display:flex;align-items:center;justify-content:center;color:var(--muted)">
        Select an agent to get started.
      </div>
    </main>
  </div>
</template>
