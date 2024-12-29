import { LoaderFunctionArgs } from "@remix-run/node";
import { useLoaderData } from "@remix-run/react";
import { format } from "date-fns";
import VideoPlayer from "~/components/video";
import { ClientOnly } from "~/utils/client-only";

const getVideoData = async (id: string) => {
  const res = await fetch(`http://localhost:8080/api/v1/videos/${id}`);

  if (!res.ok) {
    console.error(await res.text());
    throw new Error("Failed to get video details");
  }

  return res.json();
};

export async function loader(args: LoaderFunctionArgs) {
  const { id } = args.params;
  if (!id) {
    throw new Error("No video id supplied");
  }
  const videoData = await getVideoData(id);
  return videoData.data;
}

export default function VideoPage() {
  const video = useLoaderData<typeof loader>();

  return (
    <div className="p-4">
      <ClientOnly fallback={<div>Loading player...</div>}>
        {() => <VideoPlayer videoId={video.id} />}
      </ClientOnly>
      <h1 className="text-lg font-semibold">{video.title}</h1>
      <p>{video.description}</p>
      <p>{format(video.created_at, "dd MMMM, yyyy")}</p>
    </div>
  );
}
