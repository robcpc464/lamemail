#pragma once

#include <QImage>
#include <QObject>
#include <QQuickImageProvider>
#include <QSize>
#include <QString>
#include <QtQml/qqmlregistration.h>

class TrayImages : public QQuickImageProvider {
public:
    TrayImages();

    QImage requestImage(const QString& id, QSize* size, const QSize& requestedSize) override;
};

class TrayHost : public QObject {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(bool present READ present NOTIFY presentChanged)

public:
    explicit TrayHost(QObject* parent = nullptr);

    bool present() const { return m_present; }

signals:
    void presentChanged();

private:
    void setPresent(bool present);

    bool m_present = false;
};
