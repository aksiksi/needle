# ffmpeg concepts

* Packet: encoded video frame or audio slices
* Time base: 1/n, where n is the number of time units in a second
* PTS: presentation timestamp
    * The time at which a packet should be displayed, in terms of the time base
    * To find the actual time in seconds: PTS * time_base = PTS / n
* DTS: decode timestamp
    * The time at which a packet needs to be decoded
    * This must be less than or equal to the PTS for a given packet
    * DTS is important for packets that contain P-frames and B-frames, as you need to decode other packets before this one can be decoded

# Preceptual Hashing

Intro the basic ideas: http://hackerfactor.com/blog/index.php%3F/archives/432-Looks-Like-It.html

# Approach

## Problem Statement

Given two video streams, `f` and `g` consisting of `N` and `M` frames, respectively:

```
f: [ f_1 | f_2 | ... | f_N ]
g: [ g_1 | g_2 | ... | g_M ]
```

We need to find the longest pair of *similar* consecutive frames and the start and end of points of these sequences:

```
f: [ ... | f_A | f_A+1 | ... | f_B | ... ]
g: [ ... | g_A | g_A+1 | ... | g_B | ... ]
```

In the case above, we need to return `A, B` for each of `f` and `g`.

## Frame Similarity

To determine if two frames are *similar*:

1. Compute each frame's perceptual hash (we are using Blockhash-144). This is ~O(N), where N is the number of pixels in a frame.
2. Compute the Hamming distance between the hashes. O(N) in number of bits (144 in this case).
3. If the distance is less than 10 (for 144 bit hashes), we can say that the two frames are similar.

## Frame Processing

1. Iterate over 30 second slices of the source video and compute a "hash" of each frame. You can sample, e.g., 1/5 frames (i.e., 5 fps).
2. As you iterate over the hashes, check against the destination video to see there is a match. If there is, find the longest continuous sequence of matching frames and keep track of it.
2. Do the same thing for the destination video.
3. Keep repeating until the end of one of the videos.
