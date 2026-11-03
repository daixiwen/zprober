@echo creating MSI file using the WiX Toolset...

candle -arch x64 zprober.wxs
light -ext WixUIExtension -cultures:en-us -out zprober-%1.msi zprober.wixobj
candle zprober-user.wxs
light -ext WixUIExtension -cultures:en-us -out zprober-user-%1.msi zprober-user.wixobj
