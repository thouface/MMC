package com.example.mmc

import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import java.net.InetAddress

class MainActivity : FlutterActivity() {
    private val CHANNEL = "com.example.mmc/platform"
    private var nsdManager: NsdManager? = null
    private var discoveryListener: NsdManager.DiscoveryListener? = null

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, CHANNEL).setMethodCallHandler { call, result ->
            when (call.method) {
                "startDiscovery" -> startNsdDiscovery(call.argument<String>("serviceType") ?: "_thefool._tcp", result)
                "stopDiscovery" -> stopNsdDiscovery(result)
                "getDeviceInfo" -> result.success(mapOf("model" to android.os.Build.MODEL, "os" to "android-${android.os.Build.VERSION.RELEASE}"))
                else -> result.notImplemented()
            }
        }
    }

    private fun startNsdDiscovery(serviceType: String, result: MethodChannel.Result) {
        nsdManager = getSystemService(Context.NSD_SERVICE) as NsdManager
        discoveryListener = object : NsdManager.DiscoveryListener {
            override fun onDiscoveryStarted(regType: String?) {}
            override fun onDiscoveryStopped(serviceType: String?) {}
            override fun onServiceFound(serviceInfo: NsdServiceInfo?) {
                serviceInfo?.let {
                    nsdManager?.resolveService(it, object : NsdManager.ResolveListener {
                        override fun onResolveFailed(serviceInfo: NsdServiceInfo?, errorCode: Int) {}
                        override fun onServiceResolved(info: NsdServiceInfo?) {
                            info?.let {
                                val data = mapOf(
                                    "name" to it.serviceName,
                                    "type" to it.serviceType,
                                    "port" to it.port,
                                    "host" to (it.host?.hostAddress ?: "")
                                )
                                MethodChannel(flutterEngine!!.dartExecutor.binaryMessenger, CHANNEL).invokeMethod("onDeviceFound", data)
                            }
                        }
                    })
                }
            }
            override fun onServiceLost(serviceInfo: NsdServiceInfo?) {}
            override fun onStartDiscoveryFailed(serviceType: String?, errorCode: Int) { result.error("START_FAILED", errorCode.toString(), null) }
            override fun onStopDiscoveryFailed(serviceType: String?, errorCode: Int) {}
        }
        nsdManager?.discoverServices(serviceType, NsdManager.PROTOCOL_DNS_SD, discoveryListener)
        result.success(null)
    }

    private fun stopNsdDiscovery(result: MethodChannel.Result) {
        try {
            discoveryListener?.let { nsdManager?.stopServiceDiscovery(it) }
        } catch (_: Exception) {}
        result.success(null)
    }
}
