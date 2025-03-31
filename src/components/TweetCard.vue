<template>
  <div class="tweet-container">
    <div class="flex items-start gap-4">
      <!-- Profile image -->
      <img
          v-if="tweet.author.profile_image_url"
          :src="tweet.author.profile_image_url"
          alt="Profile"
          class="w-12 h-12 rounded-full"
      />
      <div v-else class="w-12 h-12 rounded-full bg-gray-200"></div>

      <div class="flex-1">
        <div class="flex items-center gap-2">
          <p class="font-semibold text-gray-900">{{ tweet.author.name }}</p>
          <p class="text-gray-500">@{{ tweet.author.username }}</p>
        </div>
        <p class="mt-3 text-gray-800">{{ tweet.content }}</p>
        <div class="mt-4 flex items-center gap-6 text-sm text-gray-500">
          <span>{{ formatDate(tweet.created_at) }}</span>
          <!-- Likes -->
          <div class="flex items-center gap-2">
            <span>❤️</span>
            <span>{{ tweet.metrics.like_count || 0 }}</span>
          </div>
          <!-- Retweets -->
          <div class="flex items-center gap-2">
            <span>🔁</span>
            <span>{{ tweet.metrics.retweet_count || 0 }}</span>
          </div>
          <!-- Replies -->
          <div class="flex items-center gap-2">
            <span>💬</span>
            <span>{{ tweet.metrics.reply_count || 0 }}</span>
          </div>
          <!-- Impressions, if available -->
          <div v-if="tweet.metrics.impression_count" class="flex items-center gap-2">
            <span>👁️</span>
            <span>{{ tweet.metrics.impression_count }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { defineProps } from 'vue';

const props = defineProps({
  tweet: {
    type: Object,
    required: true
  }
});

const formatDate = (dateString) => {
  if (!dateString) return '';
  const date = new Date(dateString);
  return date.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric'
  });
};
</script>

<style scoped>
.tweet-container {
  max-width: 600px;
  margin: 0 auto;
}
</style>
