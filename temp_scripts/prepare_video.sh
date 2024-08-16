#!/bin/bash
mkdir video
cd video
ffmpeg -hwaccel vaapi -vaapi_device /dev/dri/renderD128 -i ../$1 -vf 'format=nv12,hwupload' -c:v av1_vaapi -b:v 30M -maxrate 40M -minrate 1M -c:a libopus -b:a 196k -f webm output.webm
ffmpeg -i output.webm -c:v copy -c:a copy -f dash -seg_duration 5 -streaming 1 -use_template 1 -use_timeline 1 video.mpd
cd ..
ffmpeg -i video/output.webm -vf "thumbnail,scale=1920:1080" -frames:v 1 thumbnail.avif
ffmpeg -i thumbnail.avif -q:v 2 thumbnail.jpg