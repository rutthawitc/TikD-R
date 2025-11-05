## HLS Fallback Investigation (feature-hls-fallback)

### Issues Found and Fixed:

1. **Lack of Debugging Visibility** ✅ FIXED
   - Added comprehensive tracing throughout the download pipeline
   - Now logs: descriptor extraction, binary/HLS attempts, playlist processing, segment downloads
   - Makes it easy to diagnose where failures occur

2. **Fragile URL Resolution** ✅ FIXED
   - Created `resolve_segment_url()` helper function
   - Properly handles both absolute and relative URLs
   - Better error messages showing which segment/URL failed

3. **Improved Variant Selection** ✅ FIXED
   - Now skips blank lines between `#EXT-X-STREAM-INF` and URI
   - Uses the robust URL resolver
   - Logs bandwidth selection process

4. **Enhanced Error Context** ✅ FIXED
   - Segment download errors now show which segment failed
   - URL parsing errors include context (line number, segment type)
   - Total segment count logged on success

### Testing Recommendations:

To test with the problematic URL (https://vt.tiktok.com/ZSyBK5V4o/):

```bash
# Enable debug logging to see all trace messages
RUST_LOG=tikd_r=debug cargo run -- https://vt.tiktok.com/ZSyBK5V4o/
```

The debug output will now show:
- Whether download_url or play_url is being used
- Playlist type (master vs media)
- Variant selection (if master)
- Each segment being downloaded
- Exact error location if it fails

### Potential Remaining Issues:

- **Cookie/Auth Requirements**: Some videos may require specific cookies or authentication
- **Geo-blocking**: Videos may be restricted by region
- **Rate Limiting**: Downloading many segments quickly may trigger throttling
- **Encryption**: Videos with METHOD=AES-128 or SAMPLE-AES are not supported
