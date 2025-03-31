<template>
  <div class="container mx-auto px-4 py-8">
    <h1 class="text-2xl font-bold text-gray-900 mb-8">Twitter Dashboard</h1>

    <!-- Elon Musk Tweet -->
    <div class="bg-white p-6 rounded-xl shadow-sm border border-gray-100 mb-8">
      <h2 class="text-xl font-semibold text-gray-900 mb-4">Latest Tweet from @elonmusk</h2>

      <div v-if="elonTweet.loading" class="text-center p-4">
        <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900 mx-auto"></div>
      </div>

      <div v-else-if="elonTweet.error" class="text-center text-red-500 p-4">
        <p>{{ elonTweet.error }}</p>
        <p v-if="elonTweet.error.includes('429')" class="text-sm text-gray-500 mt-2">
          Twitter API rate limit reached. Please try again later.
        </p>
      </div>

      <TweetCard v-else-if="elonTweet.data" :tweet="elonTweet.data" />

      <div v-else class="text-center text-gray-500 p-4">
        No tweets available
      </div>
    </div>

    <!-- Donald Trump Tweet -->
    <div class="bg-white p-6 rounded-xl shadow-sm border border-gray-100">
      <h2 class="text-xl font-semibold text-gray-900 mb-4">Latest Tweet from @realDonaldTrump</h2>

      <div v-if="trumpTweet.loading" class="text-center p-4">
        <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900 mx-auto"></div>
      </div>

      <div v-else-if="trumpTweet.error" class="text-center p-4">
        <p class="text-red-500">{{ trumpTweet.error }}</p>
        <p v-if="trumpTweet.error.includes('429')" class="text-sm text-gray-500 mt-2">
          Twitter API rate limit reached. Please try again later.
        </p>
        <p v-else-if="trumpTweet.error.includes('401')" class="text-sm text-gray-500 mt-2">
          This Twitter account might be suspended or protected.
        </p>
      </div>

      <TweetCard v-else-if="trumpTweet.data" :tweet="trumpTweet.data" />

      <div v-else class="text-center text-gray-500 p-4">
        No tweets available
      </div>
    </div>
  </div>
</template>
<script setup>
import { ref, reactive, onMounted } from 'vue'
import { createClient } from '@supabase/supabase-js'
import TweetCard from './components/TweetCard.vue'
import { useToast } from 'vue-toastification'

// Create tweet state objects for each user
const elonTweet = reactive({
  data: null,
  loading: true,
  error: null
})

const trumpTweet = reactive({
  data: null,
  loading: true,
  error: null
})

// Create Supabase client using env variables
const supabaseUrl = import.meta.env.VITE_SUPABASE_URL
const supabaseKey = import.meta.env.VITE_SUPABASE_ANON_KEY
const supabase = createClient(supabaseUrl, supabaseKey)
const toast = useToast()

// Functions
const initAuth = async () => {
  const { data } = await supabase.auth.getSession()
  supabase.auth.onAuthStateChange((event, session) => {
    console.log('Auth state changed:', event, session)
  })
  return data
}

// Update the triggerFetch function to handle rate limiting
const triggerFetch = async (userId, username) => {
  try {
    console.log(`Triggering Edge Function for ${username}...`)
    const tweetState = username === 'elonmusk' ? elonTweet : trumpTweet

    // Set a loading state
    tweetState.loading = true

    const response = await fetch(`${supabaseUrl}/functions/v1/twitter-fetch`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'apikey': supabaseKey,
        'Authorization': `Bearer ${supabaseKey}`
      },
      body: JSON.stringify({ userId })
    })


    if (response.status === 429) {
      console.warn(`Rate limit reached for ${username}, using cached data if available`)
      toast.warning(`Twitter rate limit reached for ${username}. Using cached data if available.`)
      await fetchLatestTweet(username, true)
      return
    }

    if (!response.ok) {
      const errorText = await response.text();
      console.error('Error response:', errorText);
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    const data = await response.json()
    console.log(`Edge Function response for ${username}:`, data)

    if (data.success) {
      await fetchLatestTweet(username)
    } else {
      throw new Error(data.error || `Failed to fetch tweets for ${username}`)
    }
  } catch (e) {
    console.error(`Trigger error for ${username}:`, e)
    const tweetState = username === 'elonmusk' ? elonTweet : trumpTweet
    tweetState.error = e.message
    tweetState.loading = false
  }
}

// Update fetchLatestTweet to handle cached-only mode
const fetchLatestTweet = async (username, cachedOnly = false) => {
  const tweetState = username === 'elonmusk' ? elonTweet : trumpTweet

  try {
    tweetState.loading = true
    tweetState.error = null
    console.log(`Fetching tweets for ${username}...`)

    const { data, error: dbError } = await supabase
        .from('tweets')
        .select('*')
        .eq('author->>username', username)
        .order('created_at', { ascending: false })
        .limit(1)

    if (dbError) throw dbError

    console.log(`Fetched tweet for ${username}:`, data)

    if (data && data.length > 0) {
      console.log(`Tweet content for ${username}:`, data[0].content)
      console.log(`Tweet author for ${username}:`, data[0].author)
      console.log(`Tweet metrics for ${username}:`, data[0].metrics)
      tweetState.data = data[0]
    } else {
      tweetState.data = null
      // Only trigger fetch if not in cached-only mode
      if (!cachedOnly) {
        if (username === 'elonmusk') {
          await triggerFetch('44196397', 'elonmusk')
        } else if (username === 'realDonaldTrump') {
          await triggerFetch('25073877', 'realDonaldTrump')
        }
      }
    }
  } catch (e) {
    console.error(`Error fetching ${username}:`, e)
    tweetState.error = e.message
  } finally {
    tweetState.loading = false
  }
}
// Move initialization to onMounted
onMounted(async () => {
  try {
    await initAuth()
    // Fetch Elon Musk's tweets first
    await fetchLatestTweet('elonmusk')

    // Add a 5-second delay before fetching Trump's tweets
    setTimeout(async () => {
      await fetchLatestTweet('realDonaldTrump')
    }, 5000)

    // Also space out your polling intervals
    setInterval(() => triggerFetch('44196397', 'elonmusk'), 5 * 60 * 1000)
    setTimeout(() => {
      setInterval(() => triggerFetch('25073877', 'realDonaldTrump'), 5 * 60 * 1000)
    }, 2.5 * 60 * 1000) // Offset by half the interval
  } catch (err) {
    console.error('Setup error:', err)
    elonTweet.error = 'Failed to initialize: ' + err.message
    trumpTweet.error = 'Failed to initialize: ' + err.message
    elonTweet.loading = false
    trumpTweet.loading = false
  }
})
</script>

<style scoped>
.container {
  max-width: 768px;
}
</style>
<style scoped>
.tweet-container {
  max-width: 600px;
  margin: 0 auto;
}

.container {
  max-width: 768px;
}
</style>
