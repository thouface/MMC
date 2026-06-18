package com.example.mmc

import android.Manifest
import android.app.Activity
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.widget.Button
import android.widget.TextView
import android.widget.Toast
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat

class MainActivity : AppCompatActivity() {

    companion object {
        private const val TAG = "MmcDemo"
        private const val PERMISSION_REQUEST_CODE = 100

        @Volatile
        private var appContext: Context? = null

        @JvmStatic
        fun getAppContext(): Context? = appContext
    }

    private lateinit var statusText: TextView
    private lateinit var startDiscoveryButton: Button
    private lateinit var startMirrorButton: Button
    private lateinit var sendFileButton: Button

    private lateinit var mmcCore: MmcCore
    private lateinit var clipboardMonitor: ClipboardMonitor

    // Permission launcher for runtime permissions
    private val permissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions()
    ) { permissions ->
        val allGranted = permissions.all { it.value }
        if (allGranted) {
            initializeMmcCore()
        } else {
            val denied = permissions.filter { !it.value }.keys.joinToString()
            statusText.text = "Permissions denied: $denied"
            Log.e(TAG, "Permissions denied: $denied")
        }
    }

    // Activity result for MediaProjection
    private val mediaProjectionLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) { result ->
        if (result.resultCode == Activity.RESULT_OK && result.data != null) {
            // Send result to ScreenCaptureService
            val serviceIntent = Intent(this, ScreenCaptureService::class.java).apply {
                action = ScreenCaptureService.ACTION_SET_RESULT
                putExtra(ScreenCaptureService.EXTRA_RESULT_CODE, result.resultCode)
                putExtra(ScreenCaptureService.EXTRA_RESULT_DATA, result.data)
            }
            startService(serviceIntent)
            statusText.text = "Screen capture started"
        } else {
            statusText.text = "Screen capture permission denied"
            Toast.makeText(this, "Screen capture requires permission", Toast.LENGTH_SHORT).show()
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        appContext = applicationContext

        statusText = findViewById(R.id.status_text)
        startDiscoveryButton = findViewById(R.id.btn_start_discovery)
        startMirrorButton = findViewById(R.id.btn_start_mirror)
        sendFileButton = findViewById(R.id.btn_send_file)

        setupNotificationChannel()
        setupClickListeners()

        // Request runtime permissions
        requestRuntimePermissions()
    }

    override fun onDestroy() {
        super.onDestroy()
        clipboardMonitor.stopMonitoring()
        try {
            mmcCore.shutdown()
            Log.d(TAG, "MMC Core shutdown")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to shutdown MMC Core", e)
        }
        appContext = null
    }

    private fun setupClickListeners() {
        startDiscoveryButton.setOnClickListener {
            if (::mmcCore.isInitialized()) {
                val status = mmcCore.startDiscovery()
                statusText.text = "Discovery status: $status"
                Log.d(TAG, "Discovery started: $status")
            } else {
                statusText.text = "Core not initialized"
            }
        }

        startMirrorButton.setOnClickListener {
            if (::mmcCore.isInitialized()) {
                startScreenCapture()
            } else {
                statusText.text = "Core not initialized"
            }
        }

        sendFileButton.setOnClickListener {
            if (::mmcCore.isInitialized()) {
                statusText.text = "Select a file to send (TODO: integrate file picker)"
                Toast.makeText(this, "File picker integration needed", Toast.LENGTH_SHORT).show()
            } else {
                statusText.text = "Core not initialized"
            }
        }
    }

    private fun setupNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val name = getString(R.string.notification_channel_name)
            val description = getString(R.string.notification_channel_description)
            val importance = NotificationManager.IMPORTANCE_DEFAULT
            val channel = NotificationChannel("mmc_main_channel", name, importance).apply {
                this.description = description
            }
            val notificationManager = getSystemService(NotificationManager::class.java)
            notificationManager.createNotificationChannel(channel)
        }
    }

    private fun requestRuntimePermissions() {
        val permissions = mutableListOf<String>()

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS)
                != PackageManager.PERMISSION_GRANTED) {
                permissions.add(Manifest.permission.POST_NOTIFICATIONS)
            }
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.READ_MEDIA_IMAGES)
                != PackageManager.PERMISSION_GRANTED) {
                permissions.add(Manifest.permission.READ_MEDIA_IMAGES)
            }
        } else {
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.WRITE_EXTERNAL_STORAGE)
                != PackageManager.PERMISSION_GRANTED) {
                permissions.add(Manifest.permission.WRITE_EXTERNAL_STORAGE)
            }
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.READ_EXTERNAL_STORAGE)
                != PackageManager.PERMISSION_GRANTED) {
                permissions.add(Manifest.permission.READ_EXTERNAL_STORAGE)
            }
        }

        if (permissions.isNotEmpty()) {
            permissionLauncher.launch(permissions.toTypedArray())
        } else {
            initializeMmcCore()
        }
    }

    private fun initializeMmcCore() {
        try {
            mmcCore = MmcCore()
            clipboardMonitor = ClipboardMonitor.getInstance(this)

            val config = CoreConfig(
                deviceId = "android-${android.os.Build.MODEL}-${System.currentTimeMillis() % 10000}",
                deviceName = "${android.os.Build.MODEL} (MMC)",
                deviceType = DeviceType.PHONE,
                appVersion = BuildConfig.VERSION_NAME,
                logDir = filesDir.absolutePath
            )

            val status = mmcCore.init(config)
            Log.d(TAG, "Init status: $status")

            if (status == CoreStatus.OK) {
                statusText.text = "MMC Core initialized\nDevice: ${config.deviceName}"

                // Start clipboard monitoring
                clipboardMonitor.startMonitoring { text ->
                    Log.d(TAG, "Local clipboard changed: ${text.take(30)}")
                }

                // Register device for discovery
                val registerStatus = mmcCore.registerDevice(8765)
                Log.d(TAG, "Device registration: $registerStatus")

                statusText.append("\nDiscovery registered")
            } else {
                statusText.text = "Failed to initialize: $status"
                Log.e(TAG, "Core init failed: $status")
            }
        } catch (e: Exception) {
            statusText.text = "Exception: ${e.message}"
            Log.e(TAG, "Failed to initialize MMC Core", e)
        }
    }

    private fun startScreenCapture() {
        // Start the screen capture foreground service
        val serviceIntent = Intent(this, ScreenCaptureService::class.java).apply {
            action = ScreenCaptureService.ACTION_START
        }
        startForegroundService(serviceIntent)

        // Request MediaProjection permission
        val projectionManager = getSystemService(Context.MEDIA_PROJECTION_SERVICE)
                as android.media.projection.MediaProjectionManager
        mediaProjectionLauncher.launch(projectionManager.createScreenCaptureIntent())
    }

    // Called from native code to get the app context
    @JvmStatic
    fun getContext(): Context = applicationContext
}
