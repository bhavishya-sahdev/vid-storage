diesel::table! {
    video_qualities (id) {
        id -> Uuid,
        video_id -> Uuid,
        resolution -> Varchar,
        bitrate -> Varchar,
        file_path -> Varchar,
        created_at -> Timestamp,
    }
}

diesel::table! {
    videos (id) {
        id -> Uuid,
        title -> Varchar,
        description -> Nullable<Text>,
        duration -> Nullable<Float8>,
        status -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::joinable!(video_qualities -> videos (video_id));

diesel::allow_tables_to_appear_in_same_query!(video_qualities, videos,);
