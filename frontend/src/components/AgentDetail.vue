<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from 'vue'
import {
  type Agent, type AgentThreadSummary, type InboxMessage,
  runAgent, getAgentThreads, getThreadMessages,
} from '../api'

const props = defineProps<{
  agent: Agent
  workspace: string
}>()

const taskInput = ref('')
const running = ref(false)
const runError = ref('')

const threads = ref<AgentThreadSummary[]>([])
const activeThreadId = ref<string | null>(null)
const activeMessages = ref<InboxMessage[]>([])

const expandedThreadId = ref<string | null>(null)
const expandedMessages = ref<InboxMessage[]>([])

let pollTimer: ReturnType<typeof setInterval> | null = null
let pollDeadline = 0

function clearPoll() {
  if (pollTimer !== null) { clearInterval(pollTimer); pollTimer = null }
}

async function run() {
  if (!taskInput.value.trim() || running.value) return
  runError.value = ''
  running.value = true
  activeThreadId.value = null
  activeMessages.value = []

  try {
    const threadId = await runAgent(props.workspace, props.agent.name, taskInput.value.trim())
    taskInput.value = ''
    activeThreadId.value = threadId
    pollDeadline = Date.now() + 360_000
    clearPoll()
    pollTimer = setInterval(async () => {
      if (Date.now() > pollDeadline) {
        clearPoll()
        running.value = false
        runError.value = 'Timed out waiting for agent response.'
        return
      }
      try {
        const msgs = await getThreadMessages(props.workspace, threadId)
        activeMessages.value = msgs
        const terminal = msgs.find(m => m.type === 'done' || m.type === 'failed')
        if (terminal) {
          clearPoll()
          running.value = false
          threads.value = await getAgentThreads(props.workspace, props.agent.name)
        }
      } catch { /* keep polling on transient errors */ }
    }, 2000)
  } catch (e: any) {
    runError.value = e.message
    running.value = false
  }
}

async function expandThread(threadId: string) {
  if (expandedThreadId.value === threadId) {
    expandedThreadId.value = null
    expandedMessages.value = []
    return
  }
  expandedThreadId.value = threadId
  expandedMessages.value = await getThreadMessages(props.workspace, threadId)
}

function msgText(msg: InboxMessage): string {
  if (!msg.content) return ''
  if (typeof msg.content === 'string') return msg.content
  return JSON.stringify(msg.content, null, 2)
}

function fmtTime(iso: string): string {
  return new Date(iso).toLocaleString(undefined, {
    month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
  })
}

function badgeClass(type: string): string {
  if (type === 'done') return 'badge done'
  if (type === 'failed') return 'badge failed'
  return 'badge running'
}

function badgeLabel(type: string): string {
  if (type === 'done') return 'done'
  if (type === 'failed') return 'failed'
  return 'running'
}

async function loadThreads() {
  try { threads.value = await getAgentThreads(props.workspace, props.agent.name) } catch { /* ignore */ }
}

onMounted(loadThreads)

watch(() => props.agent.name, () => {
  clearPoll()
  running.value = false
  activeThreadId.value = null
  activeMessages.value = []
  expandedThreadId.value = null
  expandedMessages.value = []
  taskInput.value = ''
  runError.value = ''
  loadThreads()
})

onUnmounted(clearPoll)
</script>

<template>
  <div class="detail">
    <!-- Header -->
    <div class="detail-header">
      <div class="detail-name">{{ agent.name }}</div>
      <div class="detail-meta">
        <span>{{ agent.runtime }}</span>
        <span v-if="agent.description">{{ agent.description }}</span>
      </div>
    </div>

    <!-- Run input -->
    <div class="run-area">
      <textarea
        v-model="taskInput"
        placeholder="What should this agent do?"
        rows="3"
        :disabled="running"
        @keydown.ctrl.enter="run"
        @keydown.meta.enter="run"
      />
      <div class="run-row">
        <span class="hint">Ctrl+Enter to run</span>
        <button :disabled="running || !taskInput.trim()" @click="run">
          {{ running ? 'Running...' : 'Run' }}
        </button>
      </div>
      <div v-if="runError" class="error-msg">{{ runError }}</div>
    </div>

    <!-- Active run output -->
    <div v-if="activeThreadId" class="active-run">
      <div class="section-label">Current run</div>
      <div class="messages scroll">
        <div
          v-for="msg in activeMessages"
          :key="msg.id"
          :class="['msg', msg.type === 'request' ? 'msg-out' : 'msg-in']"
        >
          <div class="msg-content">{{ msgText(msg) }}</div>
          <div class="msg-meta">{{ msg.type }} · {{ fmtTime(msg.created_at) }}</div>
        </div>
        <div v-if="running" class="msg msg-in">
          <div class="msg-content" style="color:var(--muted)">Running...</div>
        </div>
      </div>
    </div>

    <!-- History -->
    <div class="history scroll">
      <div class="section-label" style="padding:12px 16px 4px">History</div>
      <div v-if="!threads.length" style="padding:4px 16px 12px;color:var(--muted);font-size:13px">
        No runs yet.
      </div>
      <div v-for="thread in threads" :key="thread.thread_id" class="thread-item">
        <div class="thread-row" @click="expandThread(thread.thread_id)">
          <div class="thread-content">{{ thread.first_content || '(no content)' }}</div>
          <div class="thread-right">
            <span :class="badgeClass(thread.latest_type)">{{ badgeLabel(thread.latest_type) }}</span>
            <span class="thread-time">{{ fmtTime(thread.latest_at) }}</span>
          </div>
        </div>
        <div v-if="expandedThreadId === thread.thread_id" class="thread-messages">
          <div
            v-for="msg in expandedMessages"
            :key="msg.id"
            :class="['msg', 'compact', msg.type === 'request' ? 'msg-out' : 'msg-in']"
          >
            <div class="msg-content">{{ msgText(msg) }}</div>
            <div class="msg-meta">{{ msg.type }}</div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.detail { display: flex; flex-direction: column; height: 100%; overflow: hidden; }

.detail-header { padding: 16px 20px; border-bottom: 1px solid var(--border); flex-shrink: 0; }
.detail-name { font-size: 16px; font-weight: 600; }
.detail-meta { display: flex; gap: 12px; font-size: 12px; color: var(--muted); margin-top: 2px; font-family: var(--mono); }

.run-area { padding: 16px 20px; border-bottom: 1px solid var(--border); flex-shrink: 0; }
.run-row { display: flex; align-items: center; justify-content: space-between; margin-top: 8px; }
.hint { font-size: 11px; color: var(--muted); }

.active-run { border-bottom: 1px solid var(--border); flex-shrink: 0; max-height: 260px; display: flex; flex-direction: column; }
.section-label { font-size: 11px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; color: var(--muted); }

.messages { flex: 1; overflow-y: auto; padding: 8px 16px 12px; display: flex; flex-direction: column; gap: 8px; }
.msg { max-width: 85%; }
.msg-out { align-self: flex-end; }
.msg-in { align-self: flex-start; }
.msg-content { background: var(--surface); border: 1px solid var(--border); border-radius: 8px; padding: 8px 12px; font-size: 13px; white-space: pre-wrap; word-break: break-word; }
.msg-out .msg-content { background: rgba(129,140,248,0.1); border-color: rgba(129,140,248,0.2); }
.msg.compact .msg-content { padding: 6px 10px; font-size: 12px; }
.msg-meta { font-size: 11px; color: var(--muted); margin-top: 3px; padding: 0 4px; }
.msg-out .msg-meta { text-align: right; }

.history { flex: 1; overflow-y: auto; }
.thread-item { border-bottom: 1px solid var(--border); }
.thread-row { display: flex; align-items: baseline; gap: 12px; padding: 10px 16px; cursor: pointer; transition: background 0.1s; }
.thread-row:hover { background: var(--surface-2); }
.thread-content { flex: 1; font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.thread-right { display: flex; align-items: center; gap: 8px; flex-shrink: 0; }
.thread-time { font-size: 11px; color: var(--muted); }
.thread-messages { padding: 4px 16px 12px; display: flex; flex-direction: column; gap: 6px; background: var(--surface); }
</style>
