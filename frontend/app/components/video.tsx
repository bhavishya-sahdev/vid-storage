import React, { useEffect, useRef, useState } from "react";
import Hls from "hls.js";
import {
  Play,
  Pause,
  Volume2,
  VolumeX,
  Maximize,
  Minimize,
} from "lucide-react";

interface VideoPlayerProps {
  videoId: string;
  quality?: string;
  onError?: (error: Error) => void;
  onQualityChanged?: (level: number) => void;
  initialMuted?: boolean;
  autoPlay?: boolean;
}

interface VideoState {
  isPlaying: boolean;
  isMuted: boolean;
  isFullscreen: boolean;
  progress: number;
  duration: number;
  currentQuality?: number;
  availableQualities: Array<{
    height: number;
    bitrate: number;
    level: number;
  }>;
}

const VideoPlayer: React.FC<VideoPlayerProps> = ({
  videoId,
  quality = "auto",
  onError,
  onQualityChanged,
  initialMuted = false,
  autoPlay = false,
}) => {
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const hlsRef = useRef<Hls | null>(null);

  const [state, setState] = useState<VideoState>({
    isPlaying: autoPlay,
    isMuted: initialMuted,
    isFullscreen: false,
    progress: 0,
    duration: 0,
    availableQualities: [],
  });

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;

    const hlsUrl = `http://localhost:8080/api/v1/videos/${videoId}/master.m3u8`;

    if (Hls.isSupported()) {
      const hls = new Hls({
        enableWorker: true,
        startLevel: -1, // Auto quality
        debug: process.env.NODE_ENV === "development",
      });

      hls.loadSource(hlsUrl);
      hls.attachMedia(video);
      hlsRef.current = hls;

      hls.on(Hls.Events.MANIFEST_PARSED, (_, data) => {
        const qualities = data.levels.map((level, index) => ({
          height: level.height,
          bitrate: level.bitrate,
          level: index,
        }));

        console.log(qualities);

        setState((prev) => ({
          ...prev,
          availableQualities: qualities,
        }));

        if (!state.isPlaying && !autoPlay) video.pause();
      });

      hls.on(Hls.Events.ERROR, (_, data) => {
        if (data.fatal) {
          onError?.(new Error(`HLS Error: ${data.type} - ${data.details}`));
        }
      });

      return () => {
        if (hls) {
          hls.destroy();
        }
      };
    } else if (video.canPlayType("application/vnd.apple.mpegurl")) {
      // Native HLS support (Safari)
      video.src = hlsUrl;
    }
  }, [videoId, quality, autoPlay, onError]);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;

    const onTimeUpdate = () => {
      setState((prev) => ({
        ...prev,
        progress: (video.currentTime / video.duration) * 100,
      }));
    };

    const onLoadedMetadata = () => {
      setState((prev) => ({
        ...prev,
        duration: video.duration,
      }));
    };

    const onFullscreenChange = () => {
      setState((prev) => ({
        ...prev,
        isFullscreen: Boolean(document.fullscreenElement),
      }));
    };

    video.addEventListener("timeupdate", onTimeUpdate);
    video.addEventListener("loadedmetadata", onLoadedMetadata);
    document.addEventListener("fullscreenchange", onFullscreenChange);

    return () => {
      video.removeEventListener("timeupdate", onTimeUpdate);
      video.removeEventListener("loadedmetadata", onLoadedMetadata);
      document.removeEventListener("fullscreenchange", onFullscreenChange);
    };
  }, []);

  const togglePlay = () => {
    const video = videoRef.current;
    if (!video) return;

    if (state.isPlaying) {
      video.pause();
    } else {
      video.play();
    }
    setState((prev) => ({ ...prev, isPlaying: !prev.isPlaying }));
  };

  const toggleMute = () => {
    const video = videoRef.current;
    if (!video) return;

    video.muted = !video.muted;
    setState((prev) => ({ ...prev, isMuted: !prev.isMuted }));
  };

  const toggleFullscreen = async () => {
    const video = videoRef.current;
    if (!video) return;

    try {
      if (!document.fullscreenElement) {
        await video.requestFullscreen();
      } else {
        await document.exitFullscreen();
      }
    } catch (error) {
      onError?.(new Error("Fullscreen API error"));
    }
  };

  const handleProgressClick = (e: React.MouseEvent<HTMLDivElement>) => {
    const video = videoRef.current;
    if (!video) return;

    const rect = e.currentTarget.getBoundingClientRect();
    const pos = (e.clientX - rect.left) / rect.width;
    video.currentTime = pos * video.duration;
  };

  const formatTime = (seconds: number): string => {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, "0")}`;
  };

  return (
    <div className="relative w-full aspect-video bg-black rounded-lg overflow-hidden">
      <video
        ref={videoRef}
        className="w-full h-full object-cover"
        playsInline
        muted={state.isMuted}
      />

      {/* Controls overlay */}
      <div className="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/70 to-transparent p-4">
        {/* Progress bar */}
        <div
          className="w-full h-1 bg-gray-600 rounded cursor-pointer mb-4"
          onClick={handleProgressClick}
        >
          <div
            className="h-full bg-blue-500 rounded"
            style={{ width: `${state.progress}%` }}
          />
        </div>

        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-4">
            <button
              onClick={togglePlay}
              className="p-2 hover:bg-white/20 rounded-full transition"
              type="button"
              aria-label={state.isPlaying ? "Pause" : "Play"}
            >
              {state.isPlaying ? (
                <Pause className="w-6 h-6 text-white" />
              ) : (
                <Play className="w-6 h-6 text-white" />
              )}
            </button>

            <button
              onClick={toggleMute}
              className="p-2 hover:bg-white/20 rounded-full transition"
              type="button"
              aria-label={state.isMuted ? "Unmute" : "Mute"}
            >
              {state.isMuted ? (
                <VolumeX className="w-6 h-6 text-white" />
              ) : (
                <Volume2 className="w-6 h-6 text-white" />
              )}
            </button>

            <span className="text-white text-sm">
              {formatTime(videoRef.current?.currentTime || 0)} /{" "}
              {formatTime(state.duration)}
            </span>
          </div>

          <button
            onClick={toggleFullscreen}
            className="p-2 hover:bg-white/20 rounded-full transition"
            type="button"
            aria-label={
              state.isFullscreen ? "Exit fullscreen" : "Enter fullscreen"
            }
          >
            {state.isFullscreen ? (
              <Minimize className="w-6 h-6 text-white" />
            ) : (
              <Maximize className="w-6 h-6 text-white" />
            )}
          </button>
        </div>
      </div>
    </div>
  );
};

export default VideoPlayer;
