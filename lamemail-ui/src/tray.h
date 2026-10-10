#pragma once

#include <QImage>
#include <QQuickImageProvider>
#include <QSize>
#include <QString>

class TrayImages : public QQuickImageProvider {
public:
    TrayImages();

    QImage requestImage(const QString& id, QSize* size, const QSize& requestedSize) override;
};
