import { LineChart, BarChart, Activity, Users, MessageSquare } from 'lucide-react';

export default function Home() {
  return (
    <main className="min-h-screen p-8">
      <div className="max-w-7xl mx-auto">
        <header className="mb-8">
          <h1 className="text-4xl font-bold text-gray-900">Twitter Analytics Dashboard</h1>
          <p className="text-gray-600 mt-2">Real-time insights and analytics for your Twitter account</p>
        </header>

        {/* Analytics Overview Cards */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 mb-8">
          <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-100">
            <div className="flex items-center justify-between">
              <h3 className="text-gray-700 font-medium">Total Followers</h3>
              <Users className="text-blue-500 h-5 w-5" />
            </div>
            <p className="text-3xl font-bold text-gray-900 mt-2">Loading...</p>
            <p className="text-green-500 text-sm mt-2">+2.5% from last week</p>
          </div>

          <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-100">
            <div className="flex items-center justify-between">
              <h3 className="text-gray-700 font-medium">Engagement Rate</h3>
              <Activity className="text-purple-500 h-5 w-5" />
            </div>
            <p className="text-3xl font-bold text-gray-900 mt-2">Loading...</p>
            <p className="text-red-500 text-sm mt-2">-0.8% from last week</p>
          </div>

          <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-100">
            <div className="flex items-center justify-between">
              <h3 className="text-gray-700 font-medium">Total Tweets</h3>
              <MessageSquare className="text-green-500 h-5 w-5" />
            </div>
            <p className="text-3xl font-bold text-gray-900 mt-2">Loading...</p>
            <p className="text-green-500 text-sm mt-2">+12 new tweets</p>
          </div>

          <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-100">
            <div className="flex items-center justify-between">
              <h3 className="text-gray-700 font-medium">Impressions</h3>
              <LineChart className="text-orange-500 h-5 w-5" />
            </div>
            <p className="text-3xl font-bold text-gray-900 mt-2">Loading...</p>
            <p className="text-green-500 text-sm mt-2">+5.2% from last week</p>
          </div>
        </div>

        {/* Main Analytics Section */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
          <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-100">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold text-gray-900">Engagement Overview</h3>
              <BarChart className="text-gray-400 h-5 w-5" />
            </div>
            <div className="h-80 flex items-center justify-center">
              <p className="text-gray-500">Loading chart data...</p>
            </div>
          </div>

          <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-100">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold text-gray-900">Follower Growth</h3>
              <LineChart className="text-gray-400 h-5 w-5" />
            </div>
            <div className="h-80 flex items-center justify-center">
              <p className="text-gray-500">Loading chart data...</p>
            </div>
          </div>
        </div>
      </div>
    </main>
  );
}