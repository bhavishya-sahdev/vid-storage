import type { ActionFunctionArgs, MetaFunction } from "@remix-run/node";
import { Form } from "@remix-run/react";

export const meta: MetaFunction = () => {
  return [
    { title: "New Remix App" },
    { name: "description", content: "Welcome to Remix!" },
  ];
};

const uploadToStorage = async (file: File) => {
  const formData = new FormData();
  formData.append("file", file);

  const response = await fetch("http://localhost:8080/api/v1/videos", {
    method: "POST",
    body: formData,
  });

  if (!response.ok) {
    console.error(response);
    throw new Error("Failed to upload file");
  }

  return response.json();
};

export async function action({ request }: ActionFunctionArgs) {
  const formData = await request.formData();
  const file = formData.get("file") as File;

  if (!file) {
    return { error: "No file uploaded" };
  }

  // Validate file type
  if (!file.type.startsWith("video/")) {
    return { error: "Please upload a video file" };
  }

  try {
    try {
      const uploadResult = await uploadToStorage(file);
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

export default function Index() {
  return (
    <div className="flex h-screen items-center justify-center">
      <Form method="post" encType="multipart/form-data">
        <input type="file" name="file" accept="video/*" required />
        <button type="submit">Upload</button>
      </Form>
    </div>
  );
}
