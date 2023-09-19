
1. Get event page (input URL), we need the cookie.

curl -v https://vimeo.com/event/3699988/3405ff81a0

< Set-Cookie: __cf_bm=h0ax.GXG0ilkttBZq2_aQ7tkHtvigV8uhMVJf489QZE-1695112534-0-Adk17vvEthvZua/OM3hzhOZvFw2a5ZyGgbHsqNR9DJJ3/TxsLaEhgO/Sj3iO1/OLj2QD5SRtRfuiqDfuappF0C4=; path=/; expires=Tue, 19-Sep-23 09:05:34 GMT; domain=.vimeo.com; HttpOnly; Secure; SameSite=None




2. Use the cookie to get a JWT. (Do we need the corresponding cookies?)

curl -v -H 'Cookie: __cf_bm=h0ax.GXG0ilkttBZq2_aQ7tkHtvigV8uhMVJf489QZE-1695112534-0-Adk17vvEthvZua/OM3hzhOZvFw2a5ZyGgbHsqNR9DJJ3/TxsLaEhgO/Sj3iO1/OLj2QD5SRtRfuiqDfuappF0C4=' https://vimeo.com/_next/viewer

< set-cookie: vuid=75867008.73043278; expires=Fri, 16-Sep-2033 08:36:39 GMT; Max-Age=315360000; path=/; domain=.vimeo.com; secure; SameSite=None
< set-cookie: _abexps=%7B%223063%22%3A%2240_off%22%7D; expires=Wed, 18-Sep-2024 08:36:39 GMT; Max-Age=31536000; path=/; domain=vimeo.com; SameSite=Lax
...
{"user":null,"jwt":"eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJleHAiOjE2OTUxMTM1MjAsInVzZXJfaWQiOm51bGwsImFwcF9pZCI6NTg0NzksInNjb3BlcyI6InB1YmxpYyIsInRlYW1fdXNlcl9pZCI6bnVsbH0.44Gz7aVRJ4J2RkyF_-VCoMECqLrp0w87tmlF7D5TEK4", ...}




3. Use the JWT to retrieve the `streamable_clip` config URL.

curl 'https://api.vimeo.com/live_events/3699988:3405ff81a0?fields=streamable_clip.config_url' -H 'Authorization: jwt eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJleHAiOjE2OTUxMTM1MjAsInVzZXJfaWQiOm51bGwsImFwcF9pZCI6NTg0NzksInNjb3BlcyI6InB1YmxpYyIsInRlYW1fdXNlcl9pZCI6bnVsbH0.44Gz7aVRJ4J2RkyF_-VCoMECqLrp0w87tmlF7D5TEK4'

{
    "streamable_clip": {
        "config_url": "https://player.vimeo.com/video/863077675/config?badge=0&byline=0&bypass_checks%5B0%5D=password&bypass_checks%5B1%5D=unlisted&bypass_privacy=1&collections=0&color=00adef&controls=1&default_to_hd=0&force_embed=1&fullscreen=1&h=d05b1b5231&like=0&logo=1&loop=0&muted=1&outro=nothing&playbar=1&portrait=0&privacy_banner=0&quality=540p&responsive=1&responsive_width=1&share=0&speed=1&title=0&volume=1&watch_later=0&s=3a265586665385a09d9c038d6fdc2e960a5e9c2c_1695213757"
    }
}



4. Fetch the config URL.

curl 'https://player.vimeo.com/video/863077675/config?badge=0&byline=0&bypass_checks%5B0%5D=password&bypass_checks%5B1%5D=unlisted&bypass_privacy=1&collections=0&color=00adef&controls=1&default_to_hd=0&force_embed=1&fullscreen=1&h=d05b1b5231&like=0&logo=1&loop=0&muted=1&outro=nothing&playbar=1&portrait=0&privacy_banner=0&quality=540p&responsive=1&responsive_width=1&share=0&speed=1&title=0&volume=1&watch_later=0&s=3a265586665385a09d9c038d6fdc2e960a5e9c2c_1695213757' | jq


{
  "video": {
    "id": 863077675,
    "title": "Online Livestream-Seminar mit Woltemade Hartman am 19. Semptember 2023 von 10:00 bis 17:00 Uhr",
    "width": 1920,
    "height": 1080,
    "duration": 0,
    "url": "",
    "share_url": "https://vimeo.com/863077675/d05b1b5231",
    "embed_code": "<iframe title=\"vimeo-player\" src=\"https://player.vimeo.com/video/863077675?h=d05b1b5231\" width=\"640\" height=\"360\" frameborder=\"0\"    allowfullscreen></iframe>",
    "hd": 1,
    "allow_hd": 1,
    "default_to_hd": 0,
    "privacy": "unlisted",
    "embed_permission": "public",
    "live_event": {
      "id": "64b1eb21-3dd5-41f0-8376-ef3f1cd81c2b",
      "status": "started",
      "show_viewer_count": true,
      "settings": {
        "event_schedule": true,
        "hide_live_label": false
      },
      "archive": {
        "status": "started",
        "source_url": ""
      },
      "ingest": {
        "scheduled_start_time": "2023-09-19T08:00:00+00:00",
        "width": 1920,
        "height": 1080,
        "start_time": 1695105735
      },
      "low_latency": false,
      "sessionUrl": "https://live-api.vimeocdn.com/sessions/64b1eb21-3dd5-41f0-8376-ef3f1cd81c2b?~exp=1695114000&~id=player&~sig=WZzwOAqSPtBN9c9leIPbX4JPRgBHNz37Ad-XVtFDBAc"
    },
    "version": {
      "current": null,
      "available": []
    },
    "unlisted_hash": "d05b1b5231",
    "rating": {
      "id": 3
    },
    "fps": 30,
    "bypass_token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJjbGlwX2lkIjo4NjMwNzc2NzUsImV4cCI6MTY5NTExNjY0MH0.6F3W5CO121a5wwdZvXMKZhjBEwiSgByXFT-OOmssfP4",
    "channel_layout": "stereo"
  },
}



5. Use the `video.share_url` as yt-dlp source URL.

yt-dlp "https://vimeo.com/863077675/d05b1b5231"
