#pragma once

#include <QObject>
#include <QString>
#include <QStringList>
#include <QtQml/qqmlregistration.h>

class Preferences : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(int wheelPixels READ wheelPixels WRITE setWheelPixels NOTIFY wheelPixelsChanged)
    Q_PROPERTY(bool closeToTray READ closeToTray WRITE setCloseToTray NOTIFY closeToTrayChanged)
    Q_PROPERTY(bool flaggedFirst READ flaggedFirst WRITE setFlaggedFirst NOTIFY flaggedFirstChanged)
    Q_PROPERTY(QStringList feedKinds READ feedKinds WRITE setFeedKinds NOTIFY feedKindsChanged)
    Q_PROPERTY(bool shadows READ shadows WRITE setShadows NOTIFY shadowsChanged)
    Q_PROPERTY(bool shadowsAvailable READ shadowsAvailable CONSTANT)
    Q_PROPERTY(
        bool notifications READ notifications WRITE setNotifications NOTIFY notificationsChanged)
    Q_PROPERTY(QString theme READ theme WRITE setTheme NOTIFY themeChanged)
    Q_PROPERTY(QString downloadFolder READ downloadFolder WRITE setDownloadFolder NOTIFY
            downloadFolderChanged)
    Q_PROPERTY(QString defaultDownloadFolder READ defaultDownloadFolder CONSTANT)

public:
    using QObject::QObject;

    int wheelPixels() const;
    void setWheelPixels(int pixels);
    bool closeToTray() const;
    void setCloseToTray(bool on);
    bool flaggedFirst() const;
    void setFlaggedFirst(bool on);
    QStringList feedKinds() const;
    void setFeedKinds(const QStringList& kinds);
    bool shadows() const;
    void setShadows(bool on);
    static bool shadowsAvailable();
    static void setShadowsAvailable(bool available);
    bool notifications() const;
    void setNotifications(bool on);
    static QString theme();
    void setTheme(const QString& name);
    static QString downloadFolder();
    void setDownloadFolder(const QString& folder);
    static QString defaultDownloadFolder();
    Q_INVOKABLE static QString downloads();

signals:
    void wheelPixelsChanged();
    void closeToTrayChanged();
    void flaggedFirstChanged();
    void feedKindsChanged();
    void shadowsChanged();
    void notificationsChanged();
    void themeChanged();
    void downloadFolderChanged();
};

QString downloadsPath(const QString& chosen);
