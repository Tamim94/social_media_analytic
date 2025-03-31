/*
  # Twitter Data Schema

  1. New Tables
    - `tweets`
      - `id` (uuid, primary key)
      - `tweet_id` (text, unique) - Twitter's tweet ID
      - `content` (text) - Tweet content
      - `author` (jsonb) - Author information
      - `metrics` (jsonb) - Engagement metrics
      - `created_at` (timestamptz)
      - `fetched_at` (timestamptz)
    
    - `tweet_analytics`
      - `id` (uuid, primary key)
      - `tweet_id` (uuid, references tweets)
      - `likes` (int)
      - `retweets` (int)
      - `replies` (int)
      - `impressions` (int)
      - `recorded_at` (timestamptz)

  2. Security
    - Enable RLS on both tables
    - Add policies for authenticated users
*/

-- Create tweets table
CREATE TABLE IF NOT EXISTS tweets (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tweet_id text UNIQUE NOT NULL,
  content text NOT NULL,
  author jsonb NOT NULL,
  metrics jsonb NOT NULL DEFAULT '{}',
  created_at timestamptz NOT NULL,
  fetched_at timestamptz NOT NULL DEFAULT now()
);

-- Create tweet analytics table
CREATE TABLE IF NOT EXISTS tweet_analytics (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tweet_id uuid REFERENCES tweets(id) ON DELETE CASCADE,
  likes int NOT NULL DEFAULT 0,
  retweets int NOT NULL DEFAULT 0,
  replies int NOT NULL DEFAULT 0,
  impressions int NOT NULL DEFAULT 0,
  recorded_at timestamptz NOT NULL DEFAULT now()
);

-- Enable RLS
ALTER TABLE tweets ENABLE ROW LEVEL SECURITY;
ALTER TABLE tweet_analytics ENABLE ROW LEVEL SECURITY;

-- Create policies
CREATE POLICY "Allow read access to tweets"
  ON tweets
  FOR SELECT
  TO authenticated
  USING (true);

CREATE POLICY "Allow read access to tweet analytics"
  ON tweet_analytics
  FOR SELECT
  TO authenticated
  USING (true);