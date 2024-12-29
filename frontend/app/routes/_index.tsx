import { useLoaderData, useNavigate } from "@remix-run/react";
import { formatDistance } from "date-fns";
const formatDuration = (duration: number) => {
  const minutes = Math.floor(duration / 60);
  const seconds = duration % 60;
  return `${minutes}:${seconds.toFixed(0).toString().padStart(2, "0")}`;
};

const fetchVideos = async () => {
  const response = await fetch("http://localhost:8080/api/v1/videos");

  if (!response.ok) {
    console.error(response);
    throw new Error("Failed to fetch videos");
  }

  return response.json();
};

type Meta = {
  base: string;
  page: number;
  per_page: number;
  total: number;
  total_pages: number;
};

type Quality = {
  bitrate: `{number}p`;
  created_at: Date;
  file_path: string;
  id: string;
  resolution: `${number}p`;
  video_id: string;
};

type Video = {
  created_at: Date;
  description: string | null;
  duration: number | null;
  id: string;
  qualities: Quality[];
  status: "processed" | "processing" | "failed" | "uploading";
  stream_url: string;
  thumbnail_url: string;
  title: string;
  updated_at: Date;
};

export async function loader() {
  const videos = (await fetchVideos()) as {
    meta: Meta;
    videos: Video[];
  };
  return videos;
}

export default function Homepage() {
  const { videos } = useLoaderData<typeof loader>();
  const navigate = useNavigate();

  return (
    <main className="max-w-screen-lg mx-auto p-4">
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 auto-rows-fr">
        {videos.map((video) => (
          <div
            key={video.id}
            className="space-y-2"
            role="link"
            tabIndex={0}
            onKeyDown={(e) => {
              console.log(e.key);
              if (e.key.toLowerCase() == "enter") {
                navigate(`/video/${video.id}`);
              }
            }}
            onClick={() => navigate(`/video/${video.id}`)}
          >
            <div className="object-contain rounded aspect-[16/9] overflow-hidden relative">
              <img src={video.thumbnail_url} alt={video.title} />
              <p className="absolute right-0 bottom-0 px-1 py-0.1 bg-black/50">
                {video.duration ? formatDuration(video.duration) : "—"}
              </p>
            </div>
            <div>
              <h2 className="font-semibold">{video.title}</h2>
              <p className="text-sm text-gray-500">
                {formatDistance(video.created_at, new Date(), {
                  addSuffix: true,
                })}
              </p>
            </div>
            {/* <p>{video.description}</p> */}
          </div>
        ))}
      </div>
    </main>
  );
}
