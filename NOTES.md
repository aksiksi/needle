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
