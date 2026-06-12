from django.urls import path


def health(request):
    return None


urlpatterns = [
    path("health/", health),
]
