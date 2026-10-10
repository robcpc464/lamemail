#include "tray.h"

#include <QColor>
#include <QImageReader>
#include <QPainter>
#include <QPointF>

namespace {

constexpr qreal iconSide = 256;
constexpr QPointF dotCenter(204, 60);
constexpr qreal dotRadius = 44;
constexpr qreal gapRadius = 62;

}

TrayImages::TrayImages()
    : QQuickImageProvider(QQuickImageProvider::Image)
{
}

QImage TrayImages::requestImage(const QString& id, QSize* size, const QSize& requestedSize)
{
    QImageReader reader(QStringLiteral(":/lamemail.svg"));
    if (requestedSize.isValid())
        reader.setScaledSize(requestedSize);
    QImage image = reader.read().convertToFormat(QImage::Format_ARGB32_Premultiplied);
    if (!image.isNull()) {
        const qreal scale = image.width() / iconSide;
        QPainter painter(&image);
        painter.setRenderHint(QPainter::Antialiasing);
        painter.setPen(Qt::NoPen);
        painter.setCompositionMode(QPainter::CompositionMode_Clear);
        painter.setBrush(Qt::black);
        painter.drawEllipse(dotCenter * scale, gapRadius * scale, gapRadius * scale);
        painter.setCompositionMode(QPainter::CompositionMode_SourceOver);
        painter.setBrush(QColor(QLatin1Char('#') + id));
        painter.drawEllipse(dotCenter * scale, dotRadius * scale, dotRadius * scale);
    }
    if (size)
        *size = image.size();
    return image;
}
