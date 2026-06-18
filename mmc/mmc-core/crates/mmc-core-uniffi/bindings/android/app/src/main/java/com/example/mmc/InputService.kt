package com.example.mmc

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.AccessibilityServiceInfo
import android.content.Intent
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import android.util.Log

/**
 * JNI bridge for Android input injection via AccessibilityService.
 *
 * This class implements the native methods that are called from the Rust
 * mmc-core library via JNI FFI declarations in platform_android.rs.
 *
 * The service requires the user to enable it in Android Settings under
 * Accessibility > MMC Demo > Input Service.
 */
class InputService : AccessibilityService() {

    companion object {
        private const val TAG = "MmcInputService"
        private var instance: InputService? = null

        @JvmStatic
        fun getInstance(): InputService? = instance

        /**
         * Enable input injection.
         * Must be called after the AccessibilityService is connected.
         *
         * Returns true if enabled successfully.
         */
        @JvmStatic
        external fun nativeEnable(): Boolean

        /**
         * Inject a touch event into the Android input system.
         *
         * @param touchType 0=Down, 1=Move, 2=Up, 3=Cancel
         * @param x X coordinate
         * @param y Y coordinate
         * @param pressure Pressure (0.0 to 1.0)
         * @param pointerId Pointer identifier
         * @param sequenceId Event sequence number
         * @return true on success
         */
        @JvmStatic
        external fun nativeInjectTouch(
            touchType: Int,
            x: Float,
            y: Float,
            pressure: Float,
            pointerId: Int,
            sequenceId: Long
        ): Boolean

        /**
         * Inject a key event into the Android input system.
         *
         * @param keyType 0=Down, 1=Up, 2=Text
         * @param keyCode Android key code (e.g. KeyEvent.KEYCODE_*)
         * @param sequenceId Event sequence number
         * @return true on success
         */
        @JvmStatic
        external fun nativeInjectKey(
            keyType: Int,
            keyCode: Int,
            sequenceId: Long
        ): Boolean

        /**
         * Check if input injection is enabled.
         */
        fun isEnabled(): Boolean = instance != null
    }

    private var injectionEnabled = false
    private var injectedTouchCount = 0L
    private var injectedKeyCount = 0L

    override fun onServiceConnected() {
        super.onServiceConnected()
        instance = this
        Log.d(TAG, "InputService connected")

        val info = AccessibilityServiceInfo().apply {
            eventTypes = AccessibilityEventTypes.TYPE_TOUCH_INTERACTION_END or
                    AccessibilityEventTypes.TYPE_VIEW_CLICKED
            feedbackType = AccessibilityServiceInfo.FEEDBACK_GENERIC
            notificationTimeout = 100
        }
        serviceInfo = info
    }

    override fun onAccessibilityEvent(event: android.accessibilityservice.AccessibilityEvent?) {
        // Log accessibility events for debugging
        Log.v(TAG, "Accessibility event: ${event?.eventType}")
    }

    override fun onInterrupt() {
        Log.w(TAG, "InputService interrupted")
    }

    override fun onDestroy() {
        super.onDestroy()
        instance = null
        injectionEnabled = false
        Log.d(TAG, "InputService destroyed")
    }

    /**
     * Enable input injection from remote device.
     */
    fun enableInjection(): Boolean {
        if (instance == null) {
            return false
        }
        injectionEnabled = true
        Log.d(TAG, "Input injection enabled")
        return true
    }

    /**
     * Disable input injection.
     */
    fun disableInjection() {
        injectionEnabled = false
        Log.d(TAG, "Input injection disabled")
    }

    /**
     * Check if injection is currently enabled.
     */
    fun isInjectionEnabled(): Boolean = injectionEnabled

    /**
     * Get the count of injected touch events.
     */
    fun getInjectedTouchCount(): Long = injectedTouchCount

    /**
     * Get the count of injected key events.
     */
    fun getInjectedKeyCount(): Long = injectedKeyCount

    /**
     * Dispatch a touch event from the remote device.
     * @param touchType 0=Down, 1=Move, 2=Up, 3=Cancel
     * @param x X coordinate
     * @param y Y coordinate
     * @param pressure Pressure value (0.0 to 1.0)
     * @param pointerId Pointer identifier
     * @param sequenceId Sequence number for the event
     */
    fun dispatchTouchEvent(
        touchType: Int,
        x: Float,
        y: Float,
        pressure: Float,
        pointerId: Int,
        sequenceId: Long
    ): Boolean {
        if (!injectionEnabled || instance == null) {
            return false
        }

        return try {
            val action = when (touchType) {
                0 -> MotionEvent.ACTION_DOWN
                1 -> MotionEvent.ACTION_MOVE
                2 -> MotionEvent.ACTION_UP
                3 -> MotionEvent.ACTION_CANCEL
                else -> return false
            }

            val now = android.os.SystemClock.uptimeMillis()
            val downTime = now - sequenceId.coerceAtMost(1000)

            val motionEvent = if (touchType == 1) {
                // For move events, use the existing pointer
                MotionEvent.obtain(
                    downTime, now, action,
                    1, arrayOf(MotionEvent.PointerProperties().apply {
                        id = pointerId
                        toolType = MotionEvent.TOOL_TYPE_FINGER
                    }),
                    arrayOf(MotionEvent.PointerCoords().apply {
                        this.x = x
                        this.y = y
                        this.pressure = pressure
                        this.size = 1f
                    })
                )
            } else {
                MotionEvent.obtain(
                    downTime, now, action,
                    1, arrayOf(MotionEvent.PointerProperties().apply {
                        id = pointerId
                        toolType = MotionEvent.TOOL_TYPE_FINGER
                    }),
                    arrayOf(MotionEvent.PointerCoords().apply {
                        this.x = x
                        this.y = y
                        this.pressure = pressure
                        this.size = 1f
                    })
                )
            }

            motionEvent.source = InputDevice.SOURCE_TOUCHSCREEN
            val result = dispatchMotionEvent(motionEvent)
            if (result) {
                injectedTouchCount++
            }
            result
        } catch (e: Exception) {
            Log.e(TAG, "Failed to dispatch touch event", e)
            false
        }
    }

    /**
     * Dispatch a key event from the remote device.
     * @param keyType 0=Down, 1=Up, 2=Text
     * @param keyCode Android key code
     * @param sequenceId Sequence number
     */
    fun dispatchKeyEvent(keyType: Int, keyCode: Int, sequenceId: Long): Boolean {
        if (!injectionEnabled || instance == null) {
            return false
        }

        return try {
            val action = when (keyType) {
                0 -> KeyEvent.ACTION_DOWN
                1 -> KeyEvent.ACTION_UP
                else -> return false
            }

            val now = android.os.SystemClock.uptimeMillis()
            val downTime = now - sequenceId.coerceAtMost(1000)

            val keyEvent = KeyEvent(downTime, now, action, keyCode)
            keyEvent.source = InputDevice.SOURCE_KEYBOARD

            val result = dispatchKeyEvent(keyEvent)
            if (result) {
                injectedKeyCount++
            }
            result
        } catch (e: Exception) {
            Log.e(TAG, "Failed to dispatch key event", e)
            false
        }
    }
}
