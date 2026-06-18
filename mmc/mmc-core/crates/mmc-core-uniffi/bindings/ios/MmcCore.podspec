Pod::Spec.new do |s|
  s.name             = 'MmcCore'
  s.version          = '0.1.0'
  s.summary          = 'Multi-Device Communication Core Framework for iOS'
  s.description      = <<-DESC
    MMC Core provides cross-device communication capabilities including
    file transfer, clipboard synchronization, and screen mirroring.
  DESC
  s.homepage         = 'https://github.com/example/mmc'
  s.license          = { :type => 'MIT', :file => 'LICENSE' }
  s.author           = { 'MMC Team' => 'team@mmc.example' }
  s.source           = { :git => 'https://github.com/example/mmc.git', :tag => s.version.to_s }
  
  s.ios.deployment_target = '15.0'
  s.swift_version = '5.0'
  
  s.source_files = 'MmcCore/Classes/**/*.{swift,m,h}'
  
  s.dependency 'mmc-core-uniffi'
end
