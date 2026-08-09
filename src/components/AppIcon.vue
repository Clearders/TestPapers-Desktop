<template>
  <svg class="app-icon" :class="`app-icon--${name}`" viewBox="0 0 24 24" fill="none" aria-hidden="true">
    <path
      v-for="(path, index) in iconPaths"
      :key="index"
      :d="path"
      stroke="currentColor"
      stroke-width="1.8"
      stroke-linecap="round"
      stroke-linejoin="round"
    />
  </svg>
</template>

<script setup lang="ts">
import { computed } from 'vue'

export type IconName = 'book' | 'check' | 'download' | 'moon' | 'paper' | 'settings' | 'sparkles' | 'sun' | 'upload' | 'x'

const props = defineProps<{ name: IconName }>()

const paths: Record<IconName, string[]> = {
  book: ['M4 19.5A2.5 2.5 0 0 1 6.5 17H20', 'M4 4.5A2.5 2.5 0 0 1 6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5Z'],
  check: ['M20 6 9 17l-5-5'],
  download: ['M12 3v12', 'M7 10l5 5 5-5', 'M5 21h14'],
  moon: ['M21 13.5A8.5 8.5 0 0 1 10.5 3 7 7 0 1 0 21 13.5Z'],
  paper: ['M6 2h8l4 4v16H6V2Z', 'M14 2v5h5', 'M9 13h6', 'M9 17h6'],
  settings: ['M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z', 'M19.4 15a1.8 1.8 0 0 0 .36 2l.04.04a2.1 2.1 0 0 1-3 3l-.04-.04a1.8 1.8 0 0 0-2-.36 1.8 1.8 0 0 0-1.1 1.66V21a2.1 2.1 0 0 1-4.2 0v-.06A1.8 1.8 0 0 0 8.4 19.3a1.8 1.8 0 0 0-2 .36l-.04.04a2.1 2.1 0 0 1-3-3l.04-.04a1.8 1.8 0 0 0 .36-2A1.8 1.8 0 0 0 2.1 13H2a2.1 2.1 0 0 1 0-4.2h.06A1.8 1.8 0 0 0 3.7 7.7a1.8 1.8 0 0 0-.36-2l-.04-.04a2.1 2.1 0 0 1 3-3l.04.04a1.8 1.8 0 0 0 2 .36A1.8 1.8 0 0 0 9.5 1.4V1a2.1 2.1 0 0 1 4.2 0v.06A1.8 1.8 0 0 0 14.8 2.7a1.8 1.8 0 0 0 2-.36l.04-.04a2.1 2.1 0 0 1 3 3l-.04.04a1.8 1.8 0 0 0-.36 2A1.8 1.8 0 0 0 21.1 8.5H21a2.1 2.1 0 0 1 0 4.2h-.06A1.8 1.8 0 0 0 19.4 15Z'],
  sparkles: ['M12 3l1.7 4.3L18 9l-4.3 1.7L12 15l-1.7-4.3L6 9l4.3-1.7L12 3Z', 'M5 15l.8 2.2L8 18l-2.2.8L5 21l-.8-2.2L2 18l2.2-.8L5 15Z', 'M19 14l.8 2.2L22 17l-2.2.8L19 20l-.8-2.2L16 17l2.2-.8L19 14Z'],
  sun: ['M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8Z', 'M12 2v2', 'M12 20v2', 'M4.93 4.93l1.41 1.41', 'M17.66 17.66l1.41 1.41', 'M2 12h2', 'M20 12h2', 'M4.93 19.07l1.41-1.41', 'M17.66 6.34l1.41-1.41'],
  upload: ['M12 15V3', 'M7 8l5-5 5 5', 'M5 21h14'],
  x: ['M18 6 6 18', 'M6 6l12 12']
}

const iconPaths = computed(() => paths[props.name])
</script>
