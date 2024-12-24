CREATE TABLE IF NOT EXISTS "videos"(
	"id" UUID NOT NULL PRIMARY KEY,
	"title" VARCHAR NOT NULL,
	"description" TEXT,
	"duration" FLOAT8,
	"status" VARCHAR NOT NULL,
	"created_at" TIMESTAMP NOT NULL,
	"updated_at" TIMESTAMP NOT NULL
);

CREATE TABLE IF NOT EXISTS "video_qualities"(
	"id" UUID NOT NULL PRIMARY KEY,
	"video_id" UUID NOT NULL,
	"resolution" VARCHAR NOT NULL,
	"bitrate" VARCHAR NOT NULL,
	"file_path" VARCHAR NOT NULL,
	"created_at" TIMESTAMP NOT NULL,
	FOREIGN KEY ("video_id") REFERENCES "videos"("id")
);