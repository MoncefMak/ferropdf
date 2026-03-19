"""
Minimal Django settings for the ferropdf example.

Run:
    pip install django
    cd examples/django_app
    python manage.py runserver
"""
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent

SECRET_KEY = "ferropdf-example-only-not-for-production"

DEBUG = True
ALLOWED_HOSTS = ["*"]

INSTALLED_APPS = [
    "django.contrib.contenttypes",
]

MIDDLEWARE = []

ROOT_URLCONF = "urls"

TEMPLATES = [
    {
        "BACKEND": "django.template.backends.django.DjangoTemplates",
        "DIRS": [BASE_DIR / "templates"],
        "APP_DIRS": False,
        "OPTIONS": {
            "context_processors": [],
        },
    },
]
