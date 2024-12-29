import type { ActionFunctionArgs, MetaFunction } from "@remix-run/node";
import { Form } from "@remix-run/react";
import { useRef } from "react";

export const meta: MetaFunction = () => {
  return [
    { title: "New Remix App" },
    { name: "description", content: "Welcome to Remix!" },
  ];
};

const uploadToStorage = async (formData: FormData) => {
  const response = await fetch("http://localhost:8080/api/v1/videos", {
    method: "POST",
    body: formData,
  });

  if (!response.ok) {
    console.error(await response.text());
    throw new Error("Failed to upload file");
  }

  return response.json();
};

export async function action({ request }: ActionFunctionArgs) {
  const formData = await request.formData();
  const file = formData.get("video") as File;

  if (!file) {
    return { error: "No file uploaded" };
  }

  // Validate file type
  if (!file.type.startsWith("video/")) {
    return { error: "Please upload a video file" };
  }

  try {
    try {
      console.log("Uploading file...");
      const uploadResult = await uploadToStorage(formData);
      console.log(uploadResult);
      return { success: true };
    } catch (error) {
      // console.error("Upload failed:", error);
      return { error: "Failed to upload file" };
    }
  } catch (error) {
    console.error("Upload failed:", error);
    return { error: "Failed to upload file" };
  }
}

export default function Upload() {
  const fileInput = useRef<HTMLInputElement>(null);
  return (
    <div className="flex h-screen items-center justify-center">
      <Form method="post" encType="multipart/form-data">
        <div className="flex flex-col space-y-4 w-96">
          <div className="flex flex-col">
            <label
              htmlFor="title"
              className="text-sm font-semibold text-zinc-500"
            >
              Title
            </label>
            <input
              type="text"
              name="title"
              placeholder="Title"
              required
              className="border rounded p-2"
            />
          </div>
          <div className="flex flex-col">
            <label
              htmlFor="description"
              className="text-sm font-semibold text-zinc-500"
            >
              Description
            </label>
            <textarea
              name="description"
              placeholder="Description"
              required
              className="border rounded p-2 resize-none"
            />
          </div>
          <input
            type="file"
            name="video"
            accept="video/*"
            required
            hidden
            ref={fileInput}
          />
          <div className="flex items-center border rounded">
            <button
              type="button"
              onClick={() => fileInput.current?.click()}
              className="border rounded p-2 shrink-0"
            >
              Select Video
            </button>
            <p className="px-2 line-clamp-1 text-zinc-400">
              {fileInput.current?.files?.[0]?.name}
            </p>
          </div>

          <button
            type="submit"
            className="border rounded p-2 bg-black text-white"
          >
            Upload Video
          </button>
        </div>
      </Form>
    </div>
  );
}
