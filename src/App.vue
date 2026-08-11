<template>
  <div class="app-shell">
    <a class="skip-link" href="#main-content">Skip to main content</a>
    <header class="app-header">
      <div class="brand">
        <span class="brand-mark"><AppIcon name="sparkles" /></span>
        <div><strong>TestPapers Desktop</strong><span>Local-first workspace</span></div>
      </div>
      <div class="header-actions">
        <span class="status-chip status-chip--offline"><span aria-hidden="true" /> Cloud not required</span>
        <label class="theme-control">
          <span class="sr-only">Theme</span>
          <AppIcon :name="effectiveTheme === 'dark' ? 'moon' : 'sun'" />
          <select
            :value="context?.theme.preference ?? 'system'"
            :disabled="busy || !context"
            @change="setThemePreference(($event.target as HTMLSelectElement).value as ThemePreference)"
          >
            <option value="system">System theme</option>
            <option value="light">Light theme</option>
            <option value="dark">Dark theme</option>
          </select>
        </label>
        <button class="icon-button" type="button" aria-label="Open preferences" @click="preferencesOpen = true"><AppIcon name="settings" /></button>
      </div>
    </header>

    <main id="main-content" class="main-content">
      <p v-if="error" class="alert alert--error" role="alert">{{ error }}</p>
      <div v-if="context?.warnings.length" class="alert alert--warning" role="status">
        <strong>Startup notice</strong>
        <ul><li v-for="warning in context.warnings" :key="warning">{{ warning }}</li></ul>
      </div>
      <LocalWorkspace />
    </main>

    <footer class="app-footer">
      <span>Settings: {{ context?.integrations.settingsPersistent ? 'persistent' : 'session only' }}</span>
      <span v-if="context">v{{ context.appVersion }} · {{ context.platform }}</span>
      <span>Tray: {{ context?.integrations.trayAvailable ? 'available' : 'unavailable' }}</span>
    </footer>

    <div v-if="closeRequest" class="modal-backdrop" role="presentation">
      <section class="modal" role="dialog" aria-modal="true" aria-labelledby="close-title">
        <span class="modal-icon"><AppIcon name="settings" /></span>
        <h2 id="close-title">What should closing the window do?</h2>
        <p>Your choice is remembered. You can change it later from Preferences.</p>
        <div class="modal-actions modal-actions--stacked">
          <button class="button button--primary" type="button" :disabled="busy" @click="resolveClose('tray')">Hide to tray</button>
          <button class="button button--danger" type="button" :disabled="busy" @click="resolveClose('quit')">Exit TestPapers</button>
          <button class="button" type="button" :disabled="busy" @click="resolveClose('cancel')">Cancel</button>
        </div>
      </section>
    </div>

    <div v-if="preferencesOpen" class="modal-backdrop" role="presentation" @click.self="preferencesOpen = false">
      <section class="modal" role="dialog" aria-modal="true" aria-labelledby="preferences-title">
        <button class="modal-close" type="button" aria-label="Close preferences" @click="preferencesOpen = false"><AppIcon name="x" /></button>
        <span class="modal-icon"><AppIcon name="settings" /></span>
        <h2 id="preferences-title">Preferences</h2>
        <label class="field">
          <span>When the main window closes</span>
          <select
            :value="context?.closeBehavior ?? 'ask'"
            :disabled="busy || !context"
            @change="setCloseBehavior(($event.target as HTMLSelectElement).value as CloseBehavior)"
          >
            <option value="ask">Ask me</option>
            <option value="quit">Exit the application</option>
            <option value="tray" :disabled="!context?.integrations.trayAvailable">Hide to system tray</option>
          </select>
        </label>
        <p class="field-hint">Choosing “Ask me” restores the first-close prompt.</p>
        <div class="modal-actions"><button class="button button--primary" type="button" @click="preferencesOpen = false">Done</button></div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { createDesktopShell } from './application/useDesktopShell'
import AppIcon from './components/AppIcon.vue'
import LocalWorkspace from './components/LocalWorkspace.vue'
import type { CloseBehavior, ThemePreference } from './types/shell'

const {
  context,
  closeRequest,
  preferencesOpen,
  busy,
  error,
  effectiveTheme,
  setThemePreference,
  setCloseBehavior,
  resolveClose
} = createDesktopShell()
</script>
