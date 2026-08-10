import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

import { tauriShellBridge, type ShellBridge } from '../infrastructure/tauri/shellBridge'
import type {
  CloseBehavior,
  CloseDecision,
  CloseRequestedEvent,
  DialogPreviewResult,
  EffectiveTheme,
  ExportFormat,
  ShellContext,
  ThemePreference,
  ThemeState
} from '../types/shell'

function applyTheme(theme: ThemeState) {
  document.documentElement.dataset.theme = theme.effective
  document.documentElement.style.colorScheme = theme.effective
  document.querySelector('meta[name="theme-color"]')?.setAttribute(
    'content',
    theme.effective === 'dark' ? '#11101a' : '#f5f7fb'
  )
}

export function createDesktopShell(bridge: ShellBridge = tauriShellBridge) {
  const context = ref<ShellContext | null>(null)
  const closeRequest = ref<CloseRequestedEvent | null>(null)
  const dialogPreview = ref<DialogPreviewResult | null>(null)
  const preferencesOpen = ref(false)
  const busy = ref(false)
  const error = ref('')
  const unlisteners: Array<() => void> = []
  let disposed = false

  const effectiveTheme = computed<EffectiveTheme>(() => context.value?.theme.effective ?? 'light')

  function replaceTheme(theme: ThemeState) {
    applyTheme(theme)
    if (context.value) context.value = { ...context.value, theme }
  }

  async function retain(subscription: Promise<() => void>) {
    const unlisten = await subscription
    if (disposed) unlisten()
    else unlisteners.push(unlisten)
  }

  async function initialise() {
    try {
      await Promise.all([
        bridge.onCloseRequested(payload => {
          closeRequest.value = payload
        }),
        bridge.onPreferencesRequested(() => {
          preferencesOpen.value = true
        }),
        bridge.onThemeChanged(replaceTheme),
        bridge.onDialogPreviewed(payload => {
          dialogPreview.value = payload
        })
      ].map(retain))
      context.value = await bridge.getContext()
      applyTheme(context.value.theme)
      await bridge.frontendReady()
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause)
      try {
        await bridge.frontendReady()
      } catch {
        // The native process will report the primary bootstrap failure.
      }
    }
  }

  async function run<T>(operation: () => Promise<T>): Promise<T | undefined> {
    busy.value = true
    error.value = ''
    try {
      return await operation()
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause)
      return undefined
    } finally {
      busy.value = false
    }
  }

  async function setThemePreference(preference: ThemePreference) {
    const next = await run(() => bridge.setThemePreference(preference))
    if (next) {
      context.value = next
      applyTheme(next.theme)
    }
  }

  async function setCloseBehavior(behavior: CloseBehavior) {
    const next = await run(() => bridge.setCloseBehavior(behavior))
    if (next) context.value = next
  }

  async function resolveClose(decision: CloseDecision) {
    const pending = closeRequest.value
    if (!pending) return
    const resolution = await run(() => bridge.resolveCloseRequest(pending.requestId, decision))
    if (resolution) closeRequest.value = null
  }

  async function previewQuestionImport() {
    const result = await run(() => bridge.previewQuestionImportDialog())
    if (result) dialogPreview.value = result
  }

  async function previewPaperExport(format: ExportFormat) {
    const result = await run(() => bridge.previewPaperExportDialog(format))
    if (result) dialogPreview.value = result
  }

  function teardown() {
    disposed = true
    window.removeEventListener('beforeunload', teardown)
    for (const unlisten of unlisteners.splice(0)) unlisten()
  }

  onMounted(() => {
    window.addEventListener('beforeunload', teardown, { once: true })
    void initialise()
  })
  onBeforeUnmount(teardown)

  return {
    context,
    closeRequest,
    dialogPreview,
    preferencesOpen,
    busy,
    error,
    effectiveTheme,
    setThemePreference,
    setCloseBehavior,
    resolveClose,
    previewQuestionImport,
    previewPaperExport,
    teardown
  }
}
