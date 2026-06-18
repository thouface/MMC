package com.example.mmc

import android.content.ClipboardManager
import android.content.Context
import android.util.Log

/**
 * Monitors system clipboard changes and notifies the Rust core library.
 *
 * This class listens for clipboard content changes and converts them
 * to ClipboardContent protocol messages for cross-device synchronization.
 */
class ClipboardMonitor private constructor(private val context: Context) {

    companion object {
        private const val TAG = "ClipboardMonitor"
        @Volatile
        private var instance: ClipboardMonitor? = null

        @JvmStatic
        fun getInstance(context: Context): ClipboardMonitor {
            return instance ?: synchronized(this) {
                instance ?: ClipboardMonitor(context.applicationContext).also { instance = it }
            }
        }
    }

    private var clipboardManager: ClipboardManager? = null
    private var isMonitoring = false
    private var lastClipContent: String? = null
    private var onClipboardChanged: ((String) -> Unit)? = null

    private val clipboardListener = ClipboardManager.OnPrimaryClipChangedListener {
        handleClipboardChange()
    }

    /**
     * Start monitoring clipboard changes.
     * @param callback Called when clipboard content changes
     */
    fun startMonitoring(callback: ((String) -> Unit)? = null) {
        if (isMonitoring) {
            Log.w(TAG, "Already monitoring clipboard")
            return
        }

        clipboardManager = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        clipboardManager?.addPrimaryClipChangedListener(clipboardListener)
        isMonitoring = true
        onClipboardChanged = callback
        Log.d(TAG, "Clipboard monitoring started")
    }

    /**
     * Stop monitoring clipboard changes.
     */
    fun stopMonitoring() {
        if (!isMonitoring) {
            return
        }
        clipboardManager?.removePrimaryClipChangedListener(clipboardListener)
        isMonitoring = false
        onClipboardChanged = null
        Log.d(TAG, "Clipboard monitoring stopped")
    }

    /**
     * Check if currently monitoring.
     */
    fun isMonitoring(): Boolean = isMonitoring

    /**
     * Get current clipboard text content.
     * @return Clipboard text or null if empty/not text
     */
    fun getCurrentText(): String? {
        return try {
            val clip = clipboardManager?.primaryClip
            if (clip != null && clip.itemCount > 0) {
                clip.getItemAt(0)?.text?.toString()
            } else {
                null
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to get clipboard text", e)
            null
        }
    }

    /**
     * Set clipboard text content.
     * @param text Text to set on clipboard
     */
    fun setText(text: String) {
        try {
            val clip = android.content.ClipData.newPlainText("MMC Clipboard", text)
            clipboardManager?.setPrimaryClip(clip)
            lastClipContent = text
            Log.d(TAG, "Set clipboard text: ${text.take(50)}")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to set clipboard text", e)
        }
    }

    /**
     * Check if clipboard has text content.
     */
    fun hasText(): Boolean {
        return clipboardManager?.hasPrimaryClip() == true &&
                clipboardManager?.primaryClipDescription?.hasMimeType(android.content.ClipDescription.MIMETYPE_TEXT_PLAIN) == true
    }

    /**
     * Check if clipboard has URL content.
     */
    fun hasUrl(): Boolean {
        return clipboardManager?.hasPrimaryClip() == true &&
                (clipboardManager?.primaryClipDescription?.hasMimeType(android.content.ClipDescription.MIMETYPE_TEXT_URI) == true ||
                        clipboardManager?.primaryClipDescription?.hasMimeType(android.content.ClipDescription.MIMETYPE_TEXT_PLAIN) == true)
    }

    private fun handleClipboardChange() {
        val text = getCurrentText()
        if (text != null && text != lastClipContent) {
            lastClipContent = text
            Log.d(TAG, "Clipboard changed: ${text.take(50)}")
            onClipboardChanged?.invoke(text)
        }
    }
}
