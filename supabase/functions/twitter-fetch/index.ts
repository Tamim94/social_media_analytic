// supabase/functions/twitter-fetch/index.ts

// Type definitions for better TypeScript support
interface TwitterUserData {
  id: string;
  username: string;
  name: string;
  profile_image_url: string;
}

interface TwitterMetrics {
  like_count: number;
  retweet_count: number;
  reply_count: number;
  impression_count?: number;
}

interface TwitterTweetData {
  id: string;
  text: string;
  created_at: string;
  author_id: string;
  public_metrics: TwitterMetrics;
}

interface TwitterAPIResponse {
  data: TwitterTweetData[];
  includes: {
    users: TwitterUserData[];
  };
}

interface FormattedTweet {
  tweet_id: string;
  content: string;
  created_at: string;
  author: {
    id: string;
    username: string;
    name: string;
    profile_image_url: string;
  };
  metrics: TwitterMetrics;
}

async function fetchTwitterData(userId: string): Promise<TwitterAPIResponse> {
  const url = `https://api.twitter.com/2/users/${userId}/tweets`

  const params = new URLSearchParams({
    'max_results': '5',
    'tweet.fields': 'created_at,public_metrics',
    'user.fields': 'username,name,profile_image_url',
    'expansions': 'author_id'
  })

  const twitterToken = Deno.env.get('TWITTER_BEARER_TOKEN')
  if (!twitterToken) {
    throw new Error('Missing TWITTER_BEARER_TOKEN')
  }

  console.log('Fetching tweets for user:', userId)
  const response = await fetch(`${url}?${params}`, {
    headers: {
      'Authorization': `Bearer ${twitterToken}`,
      'Content-Type': 'application/json'
    }
  })

  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Twitter API error: ${response.status} ${errorText}`)
  }
  if (response.status === 401) {
    throw new Error(`Twitter account may be suspended or protected (${response.status})`)
  }

  const data = await response.json()
  console.log('Twitter API Response:', JSON.stringify(data, null, 2))
  return data as TwitterAPIResponse
}

import { serve } from 'https://deno.land/std@0.168.0/http/server.ts'
import { createClient } from 'https://esm.sh/@supabase/supabase-js@2'

const corsHeaders = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Headers': 'authorization, x-client-info, apikey, content-type',
  'Access-Control-Allow-Methods': 'POST, GET, OPTIONS'
}

async function storeTweetsInSupabase(twitterData: TwitterAPIResponse, supabase: any): Promise<FormattedTweet[]> {
  if (!twitterData.data || !twitterData.includes || !twitterData.includes.users) {
    throw new Error('Invalid Twitter data format')
  }

  const tweets: FormattedTweet[] = twitterData.data.map(tweet => {
    const author = twitterData.includes.users.find(
        user => user.id === tweet.author_id
    )

    if (!author) {
      throw new Error(`Author not found for tweet ${tweet.id}`)
    }

    return {
      tweet_id: tweet.id,
      content: tweet.text,
      created_at: tweet.created_at,
      author: {
        id: author.id,
        username: author.username,
        name: author.name,
        profile_image_url: author.profile_image_url
      },
      metrics: {
        like_count: tweet.public_metrics.like_count,
        retweet_count: tweet.public_metrics.retweet_count,
        reply_count: tweet.public_metrics.reply_count,
        impression_count: tweet.public_metrics.impression_count || 0
      }
    }
  })

  // Insert tweets into database
  for (const tweet of tweets) {
    const { error } = await supabase
        .from('tweets')
        .upsert(tweet, { onConflict: 'tweet_id' })

    if (error) {
      throw new Error(`Failed to insert tweet: ${error.message}`)
    }
  }

  return tweets
}

// Add rate limiting and caching logic to twitter-fetch/index.ts

serve(async (req) => {
  // Handle CORS
  if (req.method === 'OPTIONS') {
    return new Response('ok', { headers: corsHeaders })
  }

  try {
    // Check for authorization header OR apikey header
    const authHeader = req.headers.get('Authorization')
    const apiKey = req.headers.get('apikey')
    let userId = '44196397' // Default to Elon Musk

    if (req.method === 'POST') {
      const body = await req.json()
      userId = body.userId || userId
    }

    if (!authHeader && !apiKey) {
      return new Response(
          JSON.stringify({ code: 401, message: "Missing authorization header" }),
          {
            status: 401,
            headers: { ...corsHeaders, 'Content-Type': 'application/json' }
          }
      )
    }

    // Initialize Supabase client
    const supabaseUrl = Deno.env.get('SUPABASE_URL')
    const supabaseKey = Deno.env.get('SUPABASE_SERVICE_ROLE_KEY')

    if (!supabaseUrl || !supabaseKey) {
      throw new Error('Missing Supabase environment variables')
    }

    const supabase = createClient(supabaseUrl, supabaseKey)

    // Check if there are recent tweets for this user already
    const { data: existingTweets } = await supabase
        .from('tweets')
        .select('*')
        .eq('author->id', userId)
        .order('created_at', { ascending: false })
        .limit(1)

    // Only fetch from Twitter API if no recent tweets or force refresh
    const forceRefresh = req.url.includes('force=true')
    const shouldFetchFromTwitter = forceRefresh || !existingTweets || existingTweets.length === 0

    if (shouldFetchFromTwitter) {
      try {
        const twitterData = await fetchTwitterData(userId)
        const formattedTweets = await storeTweetsInSupabase(twitterData, supabase)
        return new Response(
            JSON.stringify({ success: true, data: formattedTweets }),
            {
              headers: { ...corsHeaders, 'Content-Type': 'application/json' },
              status: 200
            }
        )
      } catch (twitterError) {
        // If Twitter API fails but we have existing data, return that instead
        if (existingTweets && existingTweets.length > 0) {
          return new Response(
              JSON.stringify({
                success: true,
                data: existingTweets,
                warning: `Twitter API error: ${twitterError.message}. Returning cached data.`
              }),
              {
                headers: { ...corsHeaders, 'Content-Type': 'application/json' },
                status: 200
              }
          )
        }
        // Otherwise propagate the error
        throw twitterError
      }
    } else {
      // Return cached data
      return new Response(
          JSON.stringify({
            success: true,
            data: existingTweets,
            cached: true
          }),
          {
            headers: { ...corsHeaders, 'Content-Type': 'application/json' },
            status: 200
          }
      )
    }
  } catch (error) {
    console.error('Edge function error:', error)
    return new Response(
        JSON.stringify({ success: false, error: error.message }),
        {
          headers: { ...corsHeaders, 'Content-Type': 'application/json' },
          status: error.message.includes('429') ? 429 : 400
        }
    )
  }
})
