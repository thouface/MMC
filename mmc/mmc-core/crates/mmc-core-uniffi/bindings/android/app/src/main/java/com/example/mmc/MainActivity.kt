package com.example.mmc

import android.os.Bundle
import android.util.Log
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import com.example.mmc.MmcCore
import com.example.mmc.DeviceType
import com.example.mmc.CoreConfig

class MainActivity : AppCompatActivity() {
    private lateinit var mmcCore: MmcCore
    private lateinit var statusText: TextView
    
    companion object {
        private const val TAG = "MmcDemo"
    }
    
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
        
        statusText = findViewById(R.id.status_text)
        
        try {
            initializeMmcCore()
            statusText.text = "MMC Core initialized successfully"
            Log.d(TAG, "MMC Core initialized successfully")
        } catch (e: Exception) {
            statusText.text = "Failed to initialize MMC Core: ${e.message}"
            Log.e(TAG, "Failed to initialize MMC Core", e)
        }
    }
    
    private fun initializeMmcCore() {
        mmcCore = MmcCore()
        
        val config = CoreConfig(
            deviceId = "android-device-123",
            deviceName = "Android Demo Device",
            deviceType = DeviceType.PHONE,
            appVersion = "1.0.0",
            logDir = null
        )
        
        val status = mmcCore.init(config)
        Log.d(TAG, "Init status: $status")
        
        if (status == com.example.mmc.CoreStatus.OK) {
            Log.d(TAG, "Core initialized, starting discovery...")
            val discoveryStatus = mmcCore.startDiscovery()
            Log.d(TAG, "Discovery status: $discoveryStatus")
        }
    }
    
    override fun onDestroy() {
        super.onDestroy()
        try {
            mmcCore.shutdown()
            Log.d(TAG, "MMC Core shutdown successfully")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to shutdown MMC Core", e)
        }
    }
}
