<script setup lang="ts">
import { WINDOWS } from '../types'

defineProps<{ modelValue: string }>()
defineEmits<{ (e: 'update:modelValue', v: string): void }>()
</script>

<template>
  <div class="controls">
    <span
      v-for="w in WINDOWS"
      :key="w.key"
      :class="{ on: w.key === modelValue }"
      @click="$emit('update:modelValue', w.key)"
    >{{ w.label }}</span>
  </div>
</template>
