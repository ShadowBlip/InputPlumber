<?xml version="1.0"?>
<!-- Based on dbus2markdown by Kalyzee -->
<!-- https://github.com/Kalyzee/dbus2markdown/tree/master -->

<xsl:stylesheet version="1.0"
xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
xmlns:doc="http://www.freedesktop.org/dbus/1.0/doc.dtd">
<xsl:output encoding="UTF-8" omit-xml-declaration="yes" indent="no"/>

<xsl:template name="string-replace-all">
    <xsl:param name="text" />
    <xsl:param name="replace" />
    <xsl:param name="by" />
    <xsl:choose>
        <xsl:when test="$text = '' or $replace = ''or not($replace)" >
            <!-- Prevent this routine from hanging -->
            <xsl:value-of select="$text" />
        </xsl:when>
        <xsl:when test="contains($text, $replace)">
            <xsl:value-of select="substring-before($text,$replace)" />
            <xsl:value-of select="$by" />
            <xsl:call-template name="string-replace-all">
                <xsl:with-param name="text" select="substring-after($text,$replace)" />
                <xsl:with-param name="replace" select="$replace" />
                <xsl:with-param name="by" select="$by" />
            </xsl:call-template>
        </xsl:when>
        <xsl:otherwise>
            <xsl:value-of select="$text" />
        </xsl:otherwise>
    </xsl:choose>
</xsl:template>

<xsl:template match="/">
# DBus Interface API
<xsl:for-each select="node/interface">
## <xsl:value-of select="@name"/>
  <xsl:text>&#xa;</xsl:text>
<xsl:if test="property">
### Properties
<xsl:text>&#xa;</xsl:text>
| Name | Access | Type | Description |
| --- | :---: | :---: | --- |
</xsl:if>
<xsl:for-each select="property">
  <xsl:variable name="aname">
    <xsl:call-template name="string-replace-all">
      <xsl:with-param name="text" select="@name" />
      <xsl:with-param name="replace" select="'_'" />
      <xsl:with-param name="by" select="'\_'" />
    </xsl:call-template>
  </xsl:variable>
  <xsl:variable name="asummary">
    <xsl:call-template name="string-replace-all">
      <xsl:with-param name="text" select="doc:doc/doc:summary" />
      <xsl:with-param name="replace" select="'&#xA;'" />
      <xsl:with-param name="by" select="' '" />
    </xsl:call-template>
  </xsl:variable>| **<xsl:value-of select="$aname"/>** | *<xsl:value-of select="@access"/>* | *<xsl:value-of select="@type"/>* | <xsl:value-of select="$asummary"/> |
</xsl:for-each>
### Methods
<xsl:for-each select="method">
<xsl:variable name="mname">
  <xsl:call-template name="string-replace-all">
    <xsl:with-param name="text" select="@name" />
    <xsl:with-param name="replace" select="'_'" />
    <xsl:with-param name="by" select="'\_'" />
  </xsl:call-template>
</xsl:variable>
#### <xsl:value-of select="$mname"/>
  <xsl:text>&#xa;&#xa;</xsl:text>
  <xsl:value-of select="doc:doc/doc:description/doc:para"/>
  <xsl:text>&#xa;</xsl:text>
  <xsl:if test="arg">
##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
  </xsl:if>
  <xsl:for-each select="arg">
  <xsl:variable name="aname">
    <xsl:call-template name="string-replace-all">
      <xsl:with-param name="text" select="@name" />
      <xsl:with-param name="replace" select="'_'" />
      <xsl:with-param name="by" select="'\_'" />
    </xsl:call-template>
  </xsl:variable>
  <xsl:variable name="asummary">
  <xsl:call-template name="string-replace-all">
    <xsl:with-param name="text" select="doc:doc/doc:summary" />
    <xsl:with-param name="replace" select="'&#xA;'" />
    <xsl:with-param name="by" select="' '" />
  </xsl:call-template>
  </xsl:variable>| **<xsl:value-of select="$aname"/>** | *<xsl:value-of select="@direction"/>* | *<xsl:value-of select="@type"/>* | <xsl:value-of select="$asummary"/> |
  </xsl:for-each>
  <xsl:text>&#xa;</xsl:text>
  </xsl:for-each>

### Signals
<xsl:for-each select="signal">
  <xsl:variable name="sname">
  <xsl:call-template name="string-replace-all">
    <xsl:with-param name="text" select="@name" />
    <xsl:with-param name="replace" select="'_'" />
    <xsl:with-param name="by" select="'\_'" />
  </xsl:call-template>
  </xsl:variable>
#### <xsl:value-of select="$sname"/>
  <xsl:text>&#xa;&#xa;</xsl:text>
  <xsl:value-of select="doc:doc/doc:description/doc:para"/>
  <xsl:text>&#xa;</xsl:text>
  <xsl:if test="arg">
##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
  </xsl:if>
  <xsl:for-each select="arg">
  <xsl:variable name="aname">
    <xsl:call-template name="string-replace-all">
      <xsl:with-param name="text" select="@name" />
      <xsl:with-param name="replace" select="'_'" />
      <xsl:with-param name="by" select="'\_'" />
    </xsl:call-template>
  </xsl:variable>| **<xsl:value-of select="$aname"/>** | *<xsl:value-of select="@direction"/>* | *<xsl:value-of select="@type"/>* | <xsl:value-of select="doc:doc/doc:summary"/> |
  </xsl:for-each>
  <xsl:text>&#xa;</xsl:text>
  </xsl:for-each>
</xsl:for-each>
</xsl:template>

</xsl:stylesheet> 
