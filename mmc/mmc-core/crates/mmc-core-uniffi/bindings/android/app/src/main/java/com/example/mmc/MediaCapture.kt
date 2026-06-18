package com.example.mmc

import android.app.Activity
import android.app.Activity.RESULT_OK
import android.content.Context
import android.content.Intent
import android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.view.Surface

/**
 * JNI bridge for Android screen capture via MediaProjection API.
 *
 * This class implements the native methods that are called from the Rust
 * mmc-core library via JNI FFI declarations in platform_android.rs.
 */
class MediaCapture private constructor() {

    private var mediaProjection: MediaProjection? = null
    private var captureSurface: Surface? = null
    private var isCapturing = false
    private var pendingResult: ((Boolean) -> Unit)? = null
    private var lastFrameBuffer: ByteArray? = null
    private var lastFrameWidth = 0
    private var lastFrameHeight = 0

    companion object {
        private var instance: MediaCapture? = null

        @JvmStatic
        fun getInstance(): MediaCapture {
            if (instance == null) {
                instance = MediaCapture()
            }
            return instance!!
        }
    }

    /**
     * Request screen capture permission using MediaProjection.
     * Must be called from an Activity context with a valid result callback.
     *
     * Returns true if permission was granted, false otherwise.
     */
    @JvmStatic
    external fun nativeRequestPermission(): Boolean

    /**
     * Capture a frame from the Android display surface.
     * The caller provides a buffer of sufficient size (width * height * 4 bytes).
     *
     * Returns true on success.
     */
    @JvmStatic
    external fun nativeCaptureFrame(buffer: ByteArray?, width: Int, height: Int): Boolean

    /**
     * Set the pending result callback for permission request.
     * Called from Kotlin when the permission activity returns a result.
     */
    fun setPermissionResult(resultCode: Int, data: Intent?) {
        if (resultCode == RESULT_OK && data != null) {
            val context = com.example.mmc.MainActivity.getAppContext()
            if (context != null) {
                val projectionManager = context.getSystemService(Context.MEDIA_PROJECTION_SERVICE)
                        as MediaProjectionManager
                mediaProjection = projectionManager.getMediaProjection(resultCode, data)
                mediaProjection?.registerCallback(object : MediaProjection.Callback() {
                    override fun onStop() {
                        isCapturing = false
                        captureSurface?.release()
                        captureSurface = null
                    }
                }, null)
            }
        }
    }

    /**
     * Start a new capture session.
     * @param width capture width
     * @param height capture height
     * @return true if capture started successfully
     */
    fun startCapture(width: Int, height: Int): Boolean {
        if (mediaProjection == null) {
            return false
        }

        return try {
            isCapturing = true
            true
        } catch (e: Exception) {
            false
        }
    }

    /**
     * Stop the current capture session.
     */
    fun stopCapture() {
        isCapturing = false
        captureSurface?.release()
        captureSurface = null
    }

    /**
     * Check if currently capturing.
     */
    fun isCapturing(): Boolean = isCapturing

    /**
     * Get the captured frame as RGBA bytes.
     * Returns null if no frame is available.
     */
    fun getLastFrame(): ByteArray? = lastFrameBuffer

    /**
     * Store a captured frame from native code.
     * This is called by the native capture implementation.
     */
    fun storeFrame(buffer: ByteArray, width: Int, height: Int) {
        lastFrameBuffer = buffer.copyOf()
        lastFrameWidth = width
        lastFrameHeight = height
    }
}
