// src/types.ts

export interface TweetMetrics {
    like_count: number;
    retweet_count: number;
    reply_count: number;
    impression_count: number;
}

export interface TweetAuthor {
    id: string;
    username: string;
    name: string;
    profile_image_url: string;
}

export interface Tweet {
    id: number;
    tweet_id: string;
    content: string;
    author: TweetAuthor;
    metrics: TweetMetrics;
    created_at: string;
}
